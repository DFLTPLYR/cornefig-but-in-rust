#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod display;

use defmt::{info, unwrap};
use defmt_rtt as _;
use display::PeripheralLayout;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Output};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::mode::Async;
use embassy_nrf::peripherals::{RNG, SAADC, USBD};
use embassy_nrf::saadc::{self, Saadc};
use embassy_nrf::twim::{self, Twim};
use embassy_nrf::{Peri, bind_interrupts, rng, usb};
use nrf_mpsl::Flash;
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use rmk::ble::build_ble_stack;
use rmk::config::StorageConfig;
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::display::DisplayProcessor;
use rmk::display::ssd1306::{I2CDisplayInterface, Ssd1306Async, prelude::*};
use rmk::futures::future::join3;
use rmk::input_device::adc::{AnalogEventType, NrfAdc};
use rmk::input_device::battery::BatteryProcessor;
use rmk::matrix::Matrix;
use rmk::split::peripheral::run_rmk_split_peripheral;
use rmk::storage::new_storage_for_split_peripheral;
use rmk::watchdog::Nrf52Watchdog;
use rmk::{HostResources, run_all};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<USBD>;
    SAADC => saadc::InterruptHandler;
    RNG => rng::InterruptHandler<RNG>;
    EGU0_SWI0 => nrf_sdc::mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => nrf_sdc::mpsl::ClockInterruptHandler, usb::vbus_detect::InterruptHandler;
    RADIO => nrf_sdc::mpsl::HighPrioInterruptHandler;
    TIMER0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    RTC0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    TWISPI0 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI0>;
});

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

const L2CAP_TXQ: u8 = 3;
const L2CAP_RXQ: u8 = 3;
const L2CAP_MTU: usize = 251;

fn build_sdc<'d, const N: usize>(
    p: nrf_sdc::Peripherals<'d>,
    rng: &'d mut rng::Rng<Async>,
    mpsl: &'d MultiprotocolServiceLayer,
    mem: &'d mut sdc::Mem<N>,
) -> Result<nrf_sdc::SoftdeviceController<'d>, nrf_sdc::Error> {
    sdc::Builder::new()?
        .support_adv()
        .support_peripheral()
        .support_dle_peripheral()
        .support_phy_update_peripheral()
        .support_le_2m_phy()
        .peripheral_count(1)?
        .buffer_cfg(L2CAP_MTU as u16, L2CAP_MTU as u16, L2CAP_TXQ, L2CAP_RXQ)?
        .build(p, rng, mpsl, mem)
}

fn init_adc(adc: Peri<'static, SAADC>) -> Saadc<'static, 1> {
    let config = saadc::Config::default();
    let channel_cfg = saadc::ChannelConfig::single_ended(saadc::VddhDiv5Input);
    interrupt::SAADC.set_priority(interrupt::Priority::P3);
    saadc::Saadc::new(adc, Irqs, config, [channel_cfg])
}

fn ble_addr() -> [u8; 6] {
    let ficr = embassy_nrf::pac::FICR;
    let high = u64::from(ficr.deviceid(1).read());
    let addr = high << 32 | u64::from(ficr.deviceid(0).read());
    let addr = addr | 0x0000_c000_0000_0000;
    unwrap!(addr.to_le_bytes()[..6].try_into())
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello Corne RMK Peripheral!");

    let mut nrf_config = embassy_nrf::config::Config::default();
    nrf_config.dcdc.reg0_voltage = Some(embassy_nrf::config::Reg0Voltage::_3V3);
    nrf_config.dcdc.reg0 = true;
    nrf_config.dcdc.reg1 = true;
    let p = embassy_nrf::init(nrf_config);

    let mpsl_p = mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);
    let lfclk_cfg = mpsl::raw::mpsl_clock_lfclk_cfg_t {
        source: mpsl::raw::MPSL_CLOCK_LF_SRC_RC as u8,
        rc_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_CTIV as u8,
        rc_temp_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_TEMP_CTIV as u8,
        accuracy_ppm: mpsl::raw::MPSL_DEFAULT_CLOCK_ACCURACY_PPM as u16,
        skip_wait_lfclk_started: mpsl::raw::MPSL_DEFAULT_SKIP_WAIT_LFCLK_STARTED != 0,
    };
    static MPSL: StaticCell<MultiprotocolServiceLayer> = StaticCell::new();
    static SESSION_MEM: StaticCell<mpsl::SessionMem<1>> = StaticCell::new();
    let mpsl = MPSL.init(unwrap!(mpsl::MultiprotocolServiceLayer::with_timeslots(
        mpsl_p,
        Irqs,
        lfclk_cfg,
        SESSION_MEM.init(mpsl::SessionMem::new())
    )));
    spawner.spawn(mpsl_task(&*mpsl).unwrap());

    let sdc_p = sdc::Peripherals::new(
        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24, p.PPI_CH25, p.PPI_CH26,
        p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
    );
    let mut rng = rng::Rng::new(p.RNG, Irqs);
    let mut sdc_mem = sdc::Mem::<6144>::new();
    let sdc = unwrap!(build_sdc(sdc_p, &mut rng, mpsl, &mut sdc_mem));
    let mut resources = HostResources::new();
    let stack = build_ble_stack(sdc, ble_addr(), &mut resources).await;

    let saadc = init_adc(p.SAADC);
    saadc.calibrate().await;

    let (row_pins, col_pins) = config_matrix_pins_nrf!(
        peripherals: p,
        input: [P0_22, P0_24, P1_00, P0_11],
        output: [P1_11, P1_13, P1_15, P0_02, P0_29, P0_31]
    );

    let scl = p.P0_20;
    let sda = p.P0_17;
    let mut twim_buf = [0u8; 32];
    let i2c = Twim::new(p.TWISPI0, Irqs, sda, scl, twim::Config::default(), &mut twim_buf);
    let interface = I2CDisplayInterface::new(i2c);
    let display =
        Ssd1306Async::new(interface, DisplaySize128x32, DisplayRotation::Rotate90).into_buffered_graphics_mode();
    let mut oled = DisplayProcessor::with_renderer(display, PeripheralLayout::default());

    let storage_config = StorageConfig {
        start_addr: 0xA0000,
        num_sectors: 32,
        ..Default::default()
    };
    let flash = Flash::take(mpsl, p.NVMC);
    let mut storage = new_storage_for_split_peripheral(flash, storage_config).await;

    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, 4, 6, true>::new(row_pins, col_pins, debouncer);

    let mut adc_device = NrfAdc::new(
        saadc,
        [AnalogEventType::Battery],
        [0],
        embassy_time::Duration::from_secs(12),
        None,
    );
    let mut battery_processor = BatteryProcessor::new(2000, 2806);

    let mut watchdog_runner = Nrf52Watchdog::default_runner(p.WDT);

    join3(
        run_all!(matrix, adc_device, storage, oled, watchdog_runner),
        run_all!(battery_processor),
        run_rmk_split_peripheral(0, &stack),
    )
    .await;
}
