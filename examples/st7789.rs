//! SPI example with the ST7789 using the ESP-RUST-BOARD
//!
//! Folowing pins are used:
//! GND | GND 
//! VCC | 3V3 
//! SCL | GPIO18 
//! SDA | GPIO23 
//! RST | GPIO22 
//! DC  | GPIO4 
//! CS  | GPIO2 
//! BLK | GPIO5 
//!
//! Depending on your target and the board you are using you have to change the pins.
//!
//! For this example you need to hook up an ST7789 SPI display.
//! The display will display an image on ferris the crab on a black background.

use std::thread;
use std::time::Duration;
use esp_idf_hal::delay::FreeRtos;

use embedded_hal::spi::MODE_3;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::primitives::PrimitiveStyle;

use esp_idf_hal::delay::Ets;
use esp_idf_hal::gpio::*;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::spi::*;
use esp_idf_hal::units::FromValueType;

use display_interface_spi::SPIInterfaceNoCS;

use embedded_graphics::image::*;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;

use mipidsi::{Builder, Orientation};

fn main() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let spi = peripherals.spi3;

    let mut rst = PinDriver::output(peripherals.pins.gpio22)?;
    let dc = PinDriver::output(peripherals.pins.gpio4)?;
    let mut backlight = PinDriver::output(peripherals.pins.gpio5)?;
    let sclk = peripherals.pins.gpio18;
    let sda = peripherals.pins.gpio23;
    let sdi = peripherals.pins.gpio19;
    let cs = peripherals.pins.gpio2;

    let mut delay = Ets;

    // configuring the spi interface, note that in order for the ST7789 to work, the data_mode needs to be set to MODE_3
    let config = config::Config::new()
        .baudrate(26.MHz().into())
        .data_mode(MODE_3);

    let device = SpiDeviceDriver::new_single(
        spi,
        sclk,
        sda,
        //Some(sdi),
        None::<AnyIOPin>,
        Some(cs),
        &SpiDriverConfig::new(),
        &config,
    )?;

    // display interface abstraction from SPI and DC
    let di = SPIInterfaceNoCS::new(device, dc);


    // create driver
    let mut display = Builder::st7789(di)
        .with_display_size(240, 320)
        // set default orientation
        .with_orientation(Orientation::Portrait(false))
        // initialize
        .init(&mut delay, Some(rst))
        .unwrap();

    // turn on the backlight
    backlight.set_high()?;
    // // Define colors
    // let top_color = Rgb565::new(0x01, 0x72, 0xbb);
    // let bottom_color = Rgb565::new(0x8d, 0x58, 0x25);

    // // Draw top rectangle
    // Rectangle::new(Point::new(0, 0), Size::new(240, 160))
    //     .into_styled(PrimitiveStyle::with_fill(top_color))
    //     .draw(&mut display)
    //     .unwrap();

    // // Draw bottom rectangle
    // Rectangle::new(Point::new(0, 160), Size::new(240, 160))
    //     .into_styled(PrimitiveStyle::with_fill(bottom_color))
    //     .draw(&mut display)
    //     .unwrap();

    println!("JFOX EFIS Display initialized!");
     let mut raw_image_data = ImageRawLE::new(include_bytes!("../examples/assets/jfox.raw"), 180);
     //let ferris = Image::new(&raw_image_data, Point::new(0, 0));

     // let raw_image_data = ImageRawLE::new(include_bytes!("../examples/assets/background.raw"), 86);
     let mut ferris = Image::new(&raw_image_data, Point::new((240 - 180) / 2, (320 - 61) / 2)); // Center at (77, 117)

    // draw image on black background
     display.clear(Rgb565::BLACK).unwrap();
     ferris.draw(&mut display).unwrap();
     thread::sleep(Duration::from_millis(5000));

     display.clear(Rgb565::BLACK);
     raw_image_data = ImageRawLE::new(include_bytes!("../examples/assets/background.raw"), 240);
     ferris = Image::new(&raw_image_data, Point::new(0, 0));
     ferris.draw(&mut display).unwrap();

    println!("AR Goggles");

    //  raw_image_data = ImageRawLE::new(include_bytes!("../examples/assets/roll_scale.raw"), 182);
    //  ferris = Image::new(&raw_image_data, Point::new(0, 0));
    //  ferris.draw(&mut display).unwrap();
    // println!("Image printed!");

    let mut i=0;
    loop {
        thread::sleep(Duration::from_millis(10));
         let ferris = Image::new(&raw_image_data, Point::new(0, { let tmp = i; i += 1; tmp }));
 //       display.clear(Rgb565::BLACK).unwrap();
        ferris.draw(&mut display).unwrap();
        if i>=120{
            i=1;
       }    
    }
}
