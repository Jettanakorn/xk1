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
//! This example implements a basic EFIS-like display with a sky-ground background
//! that animates pitch (vertical movement) and roll (rotation) in distinct phases,
//! and adds a fixed aircraft symbol and pitch/roll reference lines.

// Use FreeRtos::delay_ms in the loop for RTOS targets
use esp_idf_hal::delay::FreeRtos;
// For floating point trig functions and PI
use libm::{sinf, cosf};
use core::f32::consts::PI;
// For manually converting display errors
use anyhow::anyhow;


use embedded_hal::spi::MODE_3;
// Use Triangle for drawing rotated areas, PrimitiveStyle for fill colors
// Added Circle, Line, Rectangle for drawing EFIS symbology
use embedded_graphics::primitives::{PrimitiveStyle, Triangle, Circle, Line, Rectangle};
// Need Point and Size for coordinates and dimensions
use embedded_graphics::geometry::{Point, Size};


use esp_idf_hal::delay::Ets;
use esp_idf_hal::gpio::*;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::spi::*;
use esp_idf_hal::units::FromValueType;

use display_interface_spi::SPIInterfaceNoCS;

// Need Rgb565 for colors
use embedded_graphics::pixelcolor::Rgb565;
// Provides DrawTarget trait and other essentials
use embedded_graphics::prelude::*;

use mipidsi::{Builder, Orientation};

// --- Constants ---
const SCREEN_WIDTH: i32 = 240;
const SCREEN_HEIGHT: i32 = 320;
const CENTER_X: i32 = SCREEN_WIDTH / 2;
const CENTER_Y: i32 = SCREEN_HEIGHT / 2;

// Colors
const SKY_COLOR: Rgb565 = Rgb565::new(0x01, 0x72, 0xbb); // A shade of blue
const GROUND_COLOR: Rgb565 = Rgb565::new(0x8d, 0x58, 0x25); // A shade of brown/orange
const WHITE: Rgb565 = Rgb565::WHITE; // White for symbology

// Pitch sensitivity (pixels per degree) - Adjust based on desired field of view
const PITCH_PIXELS_PER_DEGREE: f32 = 4.0; // Example: 4 pixels per degree

// Animation speed (used in "both" phase)
const PITCH_SPEED_BOTH_PHASE: f32 = 0.02; // Radians per frame
const ROLL_SPEED_BOTH_PHASE: f32 = 0.015; // Radians per frame

// Max pitch/roll for animation
const MAX_PITCH_DEG: f32 = 20.0; // Max pitch up/down
const MAX_ROLL_DEG: f32 = 30.0; // Max roll left/right

// --- EFIS Symbology Constants ---
const AIRCRAFT_SYMBOL_RADIUS: u32 = 3; // Size of the center dot
const HORIZON_MARKER_OFFSET_X: i32 = 20; // Horizontal offset from center for side markers
const HORIZON_MARKER_LENGTH: u32 = 20; // Length of the side markers
const HORIZON_MARKER_THICKNESS: u32 = 4; // Thickness of the side markers

// --- Animation Phase Constants ---
const PITCH_ONLY_DURATION_FRAMES: u32 = 300; // How long Pitch-only phase lasts
const ROLL_ONLY_DURATION_FRAMES: u32 = 300; // How long Roll-only phase lasts
const BOTH_ANIMATION_DURATION_FRAMES: u32 = 400; // How long Both phase lasts

// --- Helper function for point rotation ---
/// Rotates a point around a given center by an angle in radians.
fn rotate_point(p: Point, center: Point, angle_rad: f32) -> Point {
    let dx = p.x as f32 - center.x as f32;
    let dy = p.y as f32 - center.y as f32;

    let cos_a = cosf(angle_rad);
    let sin_a = sinf(angle_rad);

    let rotated_dx = dx * cos_a - dy * sin_a;
    let rotated_dy = dx * sin_a + dy * cos_a;

    Point::new(
        (center.x as f32 + rotated_dx).round() as i32,
        (center.y as f32 + rotated_dy).round() as i32,
    )
}

fn main() -> anyhow::Result<()> {
    // --- Hardware Setup (from original example) ---
    let peripherals = Peripherals::take()?;
    let spi = peripherals.spi3;

    let mut rst = PinDriver::output(peripherals.pins.gpio22)?;
    let dc = PinDriver::output(peripherals.pins.gpio4)?;
    let mut backlight = PinDriver::output(peripherals.pins.gpio5)?;
    let sclk = peripherals.pins.gpio18;
    let sda = peripherals.pins.gpio23;
    // let sdi = peripherals.pins.gpio19; // Not used in this config
    // Corrected: Access gpio2 from the 'peripherals' variable
    let cs = peripherals.pins.gpio2;

    let mut delay = Ets; // Ets is fine for initialization delays

    // configuring the spi interface, note that in order for the ST7789 to work, the data_mode needs to be set to MODE_3
    let config = config::Config::new()
        .baudrate(26.MHz().into())
        .data_mode(MODE_3);

    let device = SpiDeviceDriver::new_single(
        spi,
        sclk,
        sda,
        None::<AnyIOPin>, // SDI pin not used
        Some(cs),
        &SpiDriverConfig::new(),
        &config,
    )?;

    // display interface abstraction from SPI and DC
    let di = SPIInterfaceNoCS::new(device, dc);

    // create driver
    // The reset sequence commented out in the original might be necessary depending on your display module
    // If display doesn't initialize, uncomment and test:
    // rst.set_low()?;
    // FreeRtos::delay_ms(100);
    // rst.set_high()?;
    // FreeRtos::delay_ms(100);
    let mut display = Builder::st7789(di)
        .with_display_size(SCREEN_WIDTH as u16, SCREEN_HEIGHT as u16)
        // set default orientation (assuming portrait, adjust if needed)
        .with_orientation(Orientation::Portrait(false)) // Portrait, normal scan direction
        // initialize
        .init(&mut delay, Some(rst))
        .unwrap();

    // turn on the backlight
    backlight.set_high()?;

    println!("JFOX AIRCRAFT EFIS Display initialized!");

    // --- Animation State Variables ---
    let mut current_phase: u8 = 0; // 0: Pitch, 1: Roll, 2: Both
    let mut phase_frame_counter: u32 = 0;
    let rotation_center = Point::new(CENTER_X, CENTER_Y);

    // --- Styles for Drawing ---
    let sky_style = PrimitiveStyle::with_fill(SKY_COLOR);
    let ground_style = PrimitiveStyle::with_fill(GROUND_COLOR);
    // Style for the white EFIS symbology (filled white)
    let white_fill_style = PrimitiveStyle::with_fill(WHITE);
    // Style for the main horizon line (stroked white, 1px thick)
    let white_line_style = PrimitiveStyle::with_stroke(WHITE, 1);


    // --- Drawing Loop ---
    loop {
        // --- Calculate Animation Values based on Phase ---
        let mut current_pitch_deg = 0.0;
        let mut current_roll_deg = 0.0;
        let phase_duration_frames: u32;

        match current_phase {
            0 => { // Pitch Only Phase
                phase_duration_frames = PITCH_ONLY_DURATION_FRAMES;
                // Animate pitch over the phase duration (full sin wave cycle)
                let angle_arg = 2.0 * PI * (phase_frame_counter as f32 / phase_duration_frames as f32);
                current_pitch_deg = MAX_PITCH_DEG * sinf(angle_arg);
                current_roll_deg = 0.0; // Roll is zero in this phase
            }
            1 => { // Roll Only Phase
                phase_duration_frames = ROLL_ONLY_DURATION_FRAMES;
                 // Animate roll over the phase duration (full sin wave cycle)
                let angle_arg = 2.0 * PI * (phase_frame_counter as f32 / phase_duration_frames as f32);
                current_roll_deg = MAX_ROLL_DEG * sinf(angle_arg);
                current_pitch_deg = 0.0; // Pitch is zero in this phase
            }
            2 => { // Both Animation Phase
                phase_duration_frames = BOTH_ANIMATION_DURATION_FRAMES;
                // Animate pitch and roll concurrently (using independent speeds like before)
                // Use the phase_frame_counter relative to the start of this phase
                current_pitch_deg = MAX_PITCH_DEG * sinf(phase_frame_counter as f32 * PITCH_SPEED_BOTH_PHASE);
                current_roll_deg = MAX_ROLL_DEG * cosf(phase_frame_counter as f32 * ROLL_SPEED_BOTH_PHASE);
            }
            _ => { // Should not happen, but handle gracefully
                phase_duration_frames = PITCH_ONLY_DURATION_FRAMES; // Default to first phase duration
                current_phase = 0;
                phase_frame_counter = 0;
            }
        }

        // Convert current roll to radians for rotation calculation
        let current_roll_rad = current_roll_deg.to_radians();

        // --- Draw Background (Sky and Ground) ---
        // Calculate the vertical pixel offset for the horizon line *in the background* based on current pitch
        let pitch_offset_y_px = current_pitch_deg * PITCH_PIXELS_PER_DEGREE;

        // Define points for the sky/ground areas, vertically shifted by pitch
        // and then rotated by roll around the screen center.
        // Cast constants to f32 for calculations involving pitch_offset_y_px
        let horizon_left_base = Point::new(
            -SCREEN_WIDTH * 2 + CENTER_X,
            (CENTER_Y as f32 + pitch_offset_y_px).round() as i32,
        );
        let horizon_right_base = Point::new(
            SCREEN_WIDTH * 3 + CENTER_X,
            (CENTER_Y as f32 + pitch_offset_y_px).round() as i32,
        );

        let sky_left_base = Point::new(
            -SCREEN_WIDTH * 2 + CENTER_X,
            (CENTER_Y as f32 + pitch_offset_y_px - SCREEN_HEIGHT as f32 * 2.0).round() as i32,
        );
        let sky_right_base = Point::new(
            SCREEN_WIDTH * 3 + CENTER_X,
            (CENTER_Y as f32 + pitch_offset_y_px - SCREEN_HEIGHT as f32 * 2.0).round() as i32,
        );

        let ground_left_base = Point::new(
            -SCREEN_WIDTH * 2 + CENTER_X,
            (CENTER_Y as f32 + pitch_offset_y_px + SCREEN_HEIGHT as f32 * 2.0).round() as i32,
        );
        let ground_right_base = Point::new(
            SCREEN_WIDTH * 3 + CENTER_X,
            (CENTER_Y as f32 + pitch_offset_y_px + SCREEN_HEIGHT as f32 * 2.0).round() as i32,
        );

        // Rotate the background points around the screen center using the current roll
        let horizon_left_rot = rotate_point(horizon_left_base, rotation_center, -current_roll_rad);
        let horizon_right_rot = rotate_point(horizon_right_base, rotation_center, -current_roll_rad);
        let sky_left_rot = rotate_point(sky_left_base, rotation_center, -current_roll_rad);
        let sky_right_rot = rotate_point(sky_right_base, rotation_center, -current_roll_rad);
        let ground_left_rot = rotate_point(ground_left_base, rotation_center, -current_roll_rad);
        let ground_right_rot = rotate_point(ground_right_base, rotation_center, -current_roll_rad);

        // Draw Sky (two triangles)
        Triangle::new(horizon_left_rot, horizon_right_rot, sky_right_rot)
            .into_styled(sky_style)
            .draw(&mut display).map_err(|e| anyhow!("Display error drawing sky triangle 1: {:?}", e))?;
        Triangle::new(horizon_left_rot, sky_right_rot, sky_left_rot)
            .into_styled(sky_style)
            .draw(&mut display).map_err(|e| anyhow!("Display error drawing sky triangle 2: {:?}", e))?;

        // Draw Ground (two triangles)
        Triangle::new(horizon_left_rot, horizon_right_rot, ground_right_rot)
            .into_styled(ground_style)
            .draw(&mut display).map_err(|e| anyhow!("Display error drawing ground triangle 1: {:?}", e))?;
        Triangle::new(horizon_left_rot, ground_right_rot, ground_left_rot)
            .into_styled(ground_style)
            .draw(&mut display).map_err(|e| anyhow!("Display error drawing ground triangle 2: {:?}", e))?;

        // --- Draw EFIS Symbology (fixed relative to screen center) ---
        // These elements remain static at the screen center.

        // Draw the main horizon line (white, 1px thick) fixed at CENTER_Y
        // Line::new(Point::new(0, CENTER_Y), Point::new(SCREEN_WIDTH, CENTER_Y))
        //     .into_styled(white_line_style)
        //     .draw(&mut display).map_err(|e| anyhow!("Display error drawing horizon line: {:?}", e))?;

        // Draw the left side marker (white, filled rectangle) fixed at CENTER_Y
        // Positioned HORIZON_MARKER_OFFSET_X left of center, length HORIZON_MARKER_LENGTH
        // Vertically centered around CENTER_Y
        Rectangle::new(
                Point::new(
                    CENTER_X - HORIZON_MARKER_OFFSET_X - HORIZON_MARKER_LENGTH as i32, // Start left of offset
                    CENTER_Y - HORIZON_MARKER_THICKNESS as i32 / 2 // Vertically centered
                ),
                Size::new(HORIZON_MARKER_LENGTH, HORIZON_MARKER_THICKNESS)
            )
            .into_styled(white_fill_style)
            .draw(&mut display).map_err(|e| anyhow!("Display error drawing left marker: {:?}", e))?;


        // Draw the right side marker (white, filled rectangle) fixed at CENTER_Y
        // Positioned HORIZON_MARKER_OFFSET_X right of center, length HORIZON_MARKER_LENGTH
        // Vertically centered around CENTER_Y
        Rectangle::new(
                Point::new(
                    CENTER_X + HORIZON_MARKER_OFFSET_X, // Start right of offset
                    CENTER_Y - HORIZON_MARKER_THICKNESS as i32 / 2 // Vertically centered
                ),
                Size::new(HORIZON_MARKER_LENGTH, HORIZON_MARKER_THICKNESS)
            )
            .into_styled(white_fill_style)
            .draw(&mut display).map_err(|e| anyhow!("Display error drawing right marker: {:?}", e))?;

        // Draw the aircraft center symbol (white, filled circle) fixed at CENTER_X, CENTER_Y
        Circle::new(Point::new(CENTER_X, CENTER_Y), AIRCRAFT_SYMBOL_RADIUS)
            .into_styled(white_fill_style)
            .draw(&mut display).map_err(|e| anyhow!("Display error drawing aircraft symbol: {:?}", e))?;


        // --- End of Drawing ---

        // --- Manage Animation Phase Transition ---
        phase_frame_counter += 1;

        if phase_frame_counter >= phase_duration_frames {
            current_phase = (current_phase + 1) % 3; // Cycle phases: 0 -> 1 -> 2 -> 0 -> ...
            phase_frame_counter = 0; // Reset counter for the new phase
             println!("Transitioning to Phase {}", current_phase); // Optional: print phase change
        }


        // Add a delay to control animation speed (e.g., ~30 FPS)
        FreeRtos::delay_ms(1); // Adjust this for overall frame rate
    }
}
