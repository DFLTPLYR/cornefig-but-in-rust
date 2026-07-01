#![no_main]
#![no_std]

use rmk::macros::rmk_central;
mod column_layout;
#[rmk_central]
mod keyboard_central {}
