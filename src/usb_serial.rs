extern crate embedded_hal as hal;
use crate::hal::usb::{UsbBus};

use usb_device::{prelude::*};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use numtoa::NumToA;

static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<stm32f0xx_hal::usb::UsbBus<stm32f0xx_hal::usb::Peripheral>>> = None;

pub struct UsbSerial {
    hours: u8,
    minutes: u8,
    serial: Option<usbd_serial::SerialPort<'static, stm32f0xx_hal::usb::UsbBus<stm32f0xx_hal::usb::Peripheral>>>,
    device: Option<UsbDevice<'static, stm32f0xx_hal::usb::UsbBus<stm32f0xx_hal::usb::Peripheral>>>
}

impl UsbSerial {
    pub fn new() -> Self {
        let usb_serial = UsbSerial {
            hours: 0,
            minutes: 0,
            serial: None,
            device: None

        };
        usb_serial
    }

    pub fn init(&mut self, usb: stm32f0xx_hal::usb::Peripheral ) {
        unsafe {    
            let usb_bus = UsbBus::new(usb);
    
            USB_BUS = Some(usb_bus);
      
            self.serial = Some(SerialPort::new(USB_BUS.as_ref().unwrap()));
      
            let usb_dev = UsbDeviceBuilder::new(USB_BUS.as_ref().unwrap(), UsbVidPid(0x16c0, 0x27dd))
                .manufacturer("FopsCorp")
                .product("Nixie watch")
                .serial_number("E621")
                .device_class(USB_CLASS_CDC)
                .build();
      
            self.device = Some(usb_dev);
          }
    }

    fn print_time(&mut self, hours: u8, minutes: u8) {
        let serial = self.serial.as_mut().unwrap();
        
        let mut write_offset = 0;
        let mut send_buffer: [u8; 6] = [b'0'; 6];
        hours.numtoa_str(10, &mut send_buffer[0..2]);
        minutes.numtoa_str(10, &mut send_buffer[3..5]);
        send_buffer[2] = b':';
        send_buffer[5] = b'\n';

        while write_offset < send_buffer.len() {
            match serial.write(&send_buffer[write_offset..send_buffer.len()]) {
                Ok(len) if len > 0 => {
                    write_offset += len;
                }
                _ => {}
            }
        }
    }

    pub fn handle(&mut self, hours: u8, minutes: u8) -> bool {
        let usb_dev = self.device.as_mut().unwrap();
        let serial = self.serial.as_mut().unwrap();

        let mut receive_buffer: [u8; 64]  = [0u8; 64];
        let mut time_set = false;

        if !usb_dev.poll(&mut [serial]) {
            return false;
        }

        match serial.read(&mut receive_buffer[..]) {
            Ok(count) if count > 0 => {
            if receive_buffer.iter().find(| &&x| x == '?' as u8) != None {
                self.print_time(hours, minutes);
            }
            else if receive_buffer.iter().find(| &&x| x == ':' as u8) != None {
                if count >= 5 {
                    self.hours = (receive_buffer[0] - 48) * 10 + receive_buffer[1] - 48;
                    self.minutes = (receive_buffer[3] - 48) * 10 + receive_buffer[4] - 48;
                    if self.hours < 24 && self.minutes < 60 {
                        time_set = true;
                        self.print_time(self.hours, self.minutes);
                    }
                }
            }
            },
            Err(UsbError::WouldBlock) => {}// No data received
            _ => {}// An error occurred
        }
        time_set
    }

    pub fn print(&mut self, string: [u8; 64], length: usize) {
        let serial = self.serial.as_mut().unwrap();
        
        let mut write_offset = 0;

        while write_offset < length {
            match serial.write(&string[write_offset..length]) {
                Ok(len) if len > 0 => {
                    write_offset += len;
                }
                _ => {}
            }
        }
    }

    //TODO: make a struct for this
    pub fn get_time(&mut self) -> (u8, u8) {
        (self.hours, self.minutes)
    }
}