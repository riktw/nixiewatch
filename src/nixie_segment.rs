extern crate embedded_hal as hal;
use crate::hal::gpio::*;
use crate::hal::prelude::*;

pub type OPIN = Pin<Output<PushPull>>;
const DIGITS: [u32;17] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F, 0x00, 0x01, 0x03, 0x07, 0x0F, 0x1F, 0x3F];

#[derive(PartialEq)]
pub enum DotStatus {
    Off,
    Digit1,
    Digit2
}

pub struct NixieDisplay {
    nixie1: OPIN,
    nixie2: OPIN,
    segments: [OPIN; 7],
    dot: OPIN,
    enable: OPIN,
    
    nixie1_value: u8,
    nixie2_value: u8,
    display_counter: u8,
    dot_status: DotStatus
}



impl NixieDisplay {
    pub fn new(nixie1: OPIN, nixie2: OPIN, segments: [OPIN;7], dot: OPIN, enable: OPIN) -> Self {
        let nixie_display = NixieDisplay {
            nixie1: nixie1, 
            nixie2: nixie2,
            segments: segments,
            dot: dot,
            enable: enable,
            nixie1_value: 2,
            nixie2_value: 7,
            display_counter: 0,
            dot_status: DotStatus::Off,
        };
        nixie_display
    }

    /// gets the bit at position `n`. Bits are numbered from 0 (least significant) to 31 (most significant).
    fn get_bit_at(input: u32, n: u8) -> bool {
        if n < 32 {
        input & (1 << n) != 0
        } else {
        false
        }
    }

    fn display_digit(&mut self, digit: u8) {
        let digit_to_display;
        self.nixie1.set_low().ok();
        self.nixie2.set_low().ok();
        self.dot.set_low().ok();
        if digit == 0 {
            digit_to_display = self.nixie1_value;
        } else {
            digit_to_display = self.nixie2_value;
        }

        let mut i = 0;
        
        for s in &mut self.segments {
            if NixieDisplay::get_bit_at(DIGITS[digit_to_display as usize], i) {
                s.set_high().ok();
              } else {
                s.set_low().ok();
              }
              i += 1;
        }

        if digit == 0 {
            self.nixie1.set_high().ok();
            self.nixie2.set_low().ok();
            if self.dot_status == DotStatus::Digit1 {
                self.dot.set_high().ok();
            }
        } else {
            self.nixie2.set_high().ok();
            self.nixie1.set_low().ok();
            if self.dot_status == DotStatus::Digit2 {
                self.dot.set_high().ok();
            }
        }

    }

    pub fn update(&mut self) {
        self.display_counter = self.display_counter.wrapping_add(1);
        if self.display_counter % 2 == 0 {
            self.display_digit(0);
        }
        else {
            self.display_digit(1);      
        }
    }

    pub fn set_digit(&mut self, digit: u8, value: u8, dot_status: DotStatus) {
        if digit == 0 {
            self.nixie1_value = value;
        } else {
            self.nixie2_value = value;
        }
        self.dot_status = dot_status;
    }

    pub fn off(&mut self) {
        self.nixie1.set_low().ok();
        self.nixie2.set_low().ok();
        self.dot.set_low().ok();
        self.enable.set_low().ok();
        for s in &mut self.segments {
            s.set_low().ok();
        }
    }
}

pub struct NixieClock {
    nixie_display: NixieDisplay,
    ticks_per_second: u32,
    current_tick: u32,
    hours: u8,
    minutes: u8,
    seconds: u8,
    display_counter: u32,
    display_status: u32,
    display_new_status: u32,
    charge_level: u8
}

// Ugly...
const SHOWTIME: u32 = 1;
const SHOWCHARGE: u32 = 2;

impl NixieClock {
    pub fn new(nixie_display: NixieDisplay, ticks_per_second: u32) -> Self {
        let nixie_clock = NixieClock{
            nixie_display: nixie_display,
            ticks_per_second: ticks_per_second,
            current_tick: 0,
            hours: 12,
            minutes: 17,
            seconds: 0,
            display_counter: 0,
            display_status: 0,
            display_new_status: 0,
            charge_level: 50
        };
        nixie_clock
    }

    pub fn set_time(&mut self, hours: u8, minutes: u8)
    {
        self.hours = hours;
        self.minutes = minutes;
    }

    pub fn get_time(&mut self) -> (u8, u8) {
        (self.hours, self.minutes)
    }

    pub fn show_time(&mut self) {
        self.display_new_status = SHOWTIME;
    }

    pub fn show_charge_done(&mut self) {
        self.display_new_status = SHOWCHARGE;
    }

    pub fn set_charge_level(&mut self, charge_level: u8) {
        self.charge_level = charge_level;
    }

    fn second_passed(&mut self) {
        self.seconds += 1;
        if self.seconds >= 60 {
            self.minutes += 1;
            self.seconds = 0;
            if self.minutes >= 60 {
                self.minutes = 0;
                self.hours += 1;
                if self.hours >= 24 {
                    self.hours = 0;
                }
            }
        }
    }

    pub fn tick(&mut self) {
        self.current_tick = if self.current_tick >= self.ticks_per_second {
            self.second_passed();
            0
        } else {
            self.current_tick + 1
        };

        if self.display_new_status != 0 {
            self.display_counter = 0;
            self.display_status = self.display_new_status;
            self.display_new_status = 0;
        }

        if self.display_counter < self.ticks_per_second * 3 {
            self.display_counter += 1;
        }

        let mut charge_value: u8 = 10 + (self.charge_level / 16); // 0 to 100 convert to 0 to 6.
        if charge_value >= 16 {charge_value = 16;}

        if self.display_counter <= self.ticks_per_second {  //Show first digit
            self.nixie_display.enable.set_high().ok();

            if self.display_status == SHOWTIME {
                self.nixie_display.set_digit(0, self.hours / 10, DotStatus::Digit1);
                self.nixie_display.set_digit(1, self.hours % 10, DotStatus::Digit1);
            } else if self.display_status == SHOWCHARGE {
                self.nixie_display.set_digit(0, charge_value, DotStatus::Off);
                self.nixie_display.set_digit(1, charge_value, DotStatus::Off);
            }
            self.nixie_display.update();

        } else if self.display_counter <=  self.ticks_per_second * 2 { //Show first digit
            self.nixie_display.enable.set_high().ok();

            if self.display_status == SHOWTIME {
                self.nixie_display.set_digit(0, self.minutes / 10, DotStatus::Digit2);
                self.nixie_display.set_digit(1, self.minutes % 10, DotStatus::Digit2);
            } else if self.display_status == SHOWCHARGE {
                self.nixie_display.set_digit(0, charge_value, DotStatus::Off);
                self.nixie_display.set_digit(1, charge_value, DotStatus::Off);
            }
            self.nixie_display.update();

        } else {
            self.nixie_display.off();
            self.display_status = 0;
        }

    }
}
