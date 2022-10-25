#![no_main]
#![no_std]

//TODO:
// 1. rename all SCHARGE_STATUS and other static muts to one clear naming scheme
// 2. enum for nixie display status

use panic_halt as _;

use stm32f0xx_hal as hal;

use crate::hal::{
  pac::{interrupt, Interrupt, Peripherals, TIM14, I2C1, EXTI},
  {i2c::I2c, delay::Delay, prelude::*},
  time::Hertz,
  adc::*,
  timers::*,
  gpio::*,
  usb::{Peripheral},
};

use cortex_m::{asm::wfi, interrupt::Mutex, peripheral::Peripherals as c_m_Peripherals};
use cortex_m_rt::entry;

use core::{cell::RefCell};

mod nixie_segment;
use nixie_segment::*;

mod usb_serial;
use usb_serial::*;

use core::sync::atomic::{AtomicU8, AtomicBool, Ordering};
use core::ops::DerefMut;

use mpu6050::*;

static HOURS: AtomicU8 = AtomicU8::new(0);
static MINUTES: AtomicU8 = AtomicU8::new(0);
static TIME_SET: AtomicBool = AtomicBool::new(false);

static MOVEMENT_DETECTED: AtomicBool = AtomicBool::new(false);

static NIXIE_DISPLAY: Mutex<RefCell<Option<NixieClock>>> = Mutex::new(RefCell::new(None));
static GINT: Mutex<RefCell<Option<Timer<TIM14>>>> = Mutex::new(RefCell::new(None));

static USB_SERIAL: Mutex<RefCell<Option<UsbSerial>>> = Mutex::new(RefCell::new(None));

static CHARGE_STATUS: Mutex<RefCell<Option<gpioa::PA1<Input<Floating>>>>> = Mutex::new(RefCell::new(None));

static BATTERY_VOLTAGE: Mutex<RefCell<Option<gpioa::PA0<Analog>>>> = Mutex::new(RefCell::new(None));
static ADC: Mutex<RefCell<Option<Adc<>>>> = Mutex::new(RefCell::new(None));

static MPU: Mutex<RefCell<Option<mpu6050::Mpu6050<I2c<I2C1,
gpiob::PB6<Alternate<AF1>>,
gpiob::PB7<Alternate<AF1>>>>
>>> = Mutex::new(RefCell::new(None));

// Make external interrupt registers globally available
static EINT: Mutex<RefCell<Option<EXTI>>> = Mutex::new(RefCell::new(None));

// Interrupt from IMU that movement was detected
#[interrupt]
fn EXTI4_15() {
  static mut EXINT: Option<EXTI> = None;
  static mut XMPU: Option<mpu6050::Mpu6050<I2c<I2C1,
  gpiob::PB6<Alternate<AF1>>,
  gpiob::PB7<Alternate<AF1>>>>> = None;

  let exti = EXINT.get_or_insert_with(|| {
    cortex_m::interrupt::free(|cs| {
      EINT.borrow(cs).replace(None).unwrap()
    })
  });

  let mpu = XMPU.get_or_insert_with(|| {
    cortex_m::interrupt::free(|cs| {
      MPU.borrow(cs).replace(None).unwrap()
    })
  });

  if mpu.get_motion_detected().unwrap() {
    MOVEMENT_DETECTED.store(true, Ordering::Relaxed);
  }

  exti.pr.write(|w| w.pif4().set_bit());

}

// Define an interupt handler, i.e. function to call when interrupt occurs. Here if our external
// interrupt trips when the timer timed out
#[interrupt]
fn TIM14() {
  //change to if let (&mut Some(ref mut nixie1), &mut Some(ref mut nix?
  static mut INT: Option<Timer<TIM14>> = None;
  static mut SNIXIEDISPLAY: Option<NixieClock> = None;
  static mut SCHARGE_STATUS: Option<gpioa::PA1<Input<Floating>>> = None;
  static mut COUNTER: u8 = 0;

  static mut SBATTERY_VOLTAGE: Option<gpioa::PA0<Analog>> = None;
  static mut SADC: Option<Adc<>> = None;

  let int = INT.get_or_insert_with(|| {
    cortex_m::interrupt::free(|cs| {
      GINT.borrow(cs).replace(None).unwrap()
    })
  });

  let nixie_clock = SNIXIEDISPLAY.get_or_insert_with(|| {
    cortex_m::interrupt::free(|cs| {
      NIXIE_DISPLAY.borrow(cs).replace(None).unwrap()
    })
  });

  let charge_status = SCHARGE_STATUS.get_or_insert_with(|| {
    cortex_m::interrupt::free(|cs| {
      CHARGE_STATUS.borrow(cs).replace(None).unwrap()
    })
  });

  let adc = SADC.get_or_insert_with(|| {
    cortex_m::interrupt::free(|cs| {
      ADC.borrow(cs).replace(None).unwrap()
    })
  });


  let battery_voltage = SBATTERY_VOLTAGE.get_or_insert_with(|| {
    cortex_m::interrupt::free(|cs| {
      BATTERY_VOLTAGE.borrow(cs).replace(None).unwrap()
    })
  });



  nixie_clock.tick();
  if TIME_SET.load(Ordering::Relaxed) {
    TIME_SET.store(false, Ordering::Relaxed);
    nixie_clock.set_time(HOURS.load(Ordering::Relaxed), MINUTES.load(Ordering::Relaxed))
  } else {
    let (hours, minutes) = nixie_clock.get_time();
    HOURS.store(hours, Ordering::Relaxed);
    MINUTES.store(minutes, Ordering::Relaxed);
  }

  let mut battery_charge: u16 = adc.read(battery_voltage).unwrap();
  battery_charge = battery_charge - 2050; // half the voltage, 3.5 to 4.2V becomes 1.75 to 2.1. Remove offset.
  battery_charge = battery_charge / 4; // 0 to 350mV is around 0 to 400.
  nixie_clock.set_charge_level(battery_charge as u8);


  *COUNTER = *COUNTER + 1;
  if *COUNTER > 50 {
    *COUNTER = 0;


    if MOVEMENT_DETECTED.load(Ordering::Relaxed) {
      MOVEMENT_DETECTED.store(false, Ordering::Relaxed);
      nixie_clock.show_time();
    } else if charge_status.is_high().unwrap() {
      nixie_clock.show_charge_done();
    }


    nixie_clock.show_charge_done(); //test


    /* 
    let mut hello_world: [u8; 64] = [0; 64];
    let hello_string = b"Motion:  \n";
    hello_world[0..hello_string.len()].clone_from_slice(hello_string);
    let acc = mpu.get_acc_angles().unwrap();
    let roll = (acc[0] * 1000.0) as i32;
    let yaw = (acc[1] * 1000.0) as i32;
    roll.numtoa(10, &mut hello_world[hello_string.len()-10..hello_string.len()-6]);
    yaw.numtoa(10, &mut hello_world[hello_string.len()-6..hello_string.len()-1]);

    cortex_m::interrupt::free(|cs| {
      if let (&mut Some(ref mut usb_serial), ) = (
        USB_SERIAL.borrow(cs).borrow_mut().deref_mut(),
      ) {
        usb_serial.print(hello_world, hello_string.len());
      }
    });*/


  }

  int.wait().ok();
}

#[interrupt]
fn USB() {
  cortex_m::interrupt::free(|cs| {
    if let (&mut Some(ref mut usb_serial), ) = (
      USB_SERIAL.borrow(cs).borrow_mut().deref_mut(),
    ) {
      if usb_serial.handle(HOURS.load(Ordering::Relaxed), MINUTES.load(Ordering::Relaxed)) {
        let (hours, minutes) = usb_serial.get_time();
        HOURS.store(hours, Ordering::Relaxed);
        MINUTES.store(minutes, Ordering::Relaxed);
        TIME_SET.store(true, Ordering::Relaxed);
      }
    }
  });
}



#[entry]
fn main() -> ! {
  if let (Some(mut p), Some(cp)) = (Peripherals::take(), c_m_Peripherals::take()) {
    cortex_m::interrupt::free(move |cs| {

      let rcc = p.RCC;
      rcc.apb2enr.modify(|_, w| w.syscfgen().set_bit());

      let mut rcc = rcc
        .configure()
        .hse(12.mhz(), stm32f0xx_hal::rcc::HSEBypassMode::NotBypassed)
        .enable_crs(p.CRS)
        .sysclk(12.mhz())
        .pclk(12.mhz())
        .usbsrc(stm32f0xx_hal::rcc::USBClockSource::HSI48)
        .freeze(&mut p.FLASH);


      let gpioa = p.GPIOA.split(&mut rcc);
      let gpiob = p.GPIOB.split(&mut rcc);
      let syscfg = p.SYSCFG;
      let exti = p.EXTI;

      //Setup PB4 as external IRQ for MPU as per https://github.com/stm32-rs/stm32f0xx-hal/blob/master/examples/led_hal_button_irq.rs
      let _ = gpiob.pb4.into_pull_down_input(cs);
      syscfg.exticr2.modify(|_, w| unsafe { w.exti4().bits(1) });
      exti.imr.modify(|_, w| w.mr4().set_bit());
      exti.rtsr.modify(|_, w| w.tr4().set_bit());

      let mut nixie1 = gpioa.pa8.into_push_pull_output(cs);
      let mut nixie2 = gpioa.pa9.into_push_pull_output(cs);

      let nixie_a = gpioa.pa4.into_push_pull_output(cs);
      let nixie_b = gpioa.pa3.into_push_pull_output(cs);
      let nixie_c = gpiob.pb1.into_push_pull_output(cs);
      let nixie_d = gpioa.pa10.into_push_pull_output(cs);
      let nixie_e = gpiob.pb3.into_push_pull_output(cs);
      let nixie_f = gpioa.pa5.into_push_pull_output(cs);
      let nixie_g = gpioa.pa6.into_push_pull_output(cs);
      let mut nixie_dp = gpioa.pa7.into_push_pull_output(cs);
      
      let mut hv_enable = gpioa.pa2.into_push_pull_output(cs);

      let charge_status = gpioa.pa1.into_floating_input(cs);

      //initial states
      nixie_dp.set_low().ok();
      nixie1.set_high().ok();
      nixie2.set_low().ok();
      hv_enable.set_low().ok();

      let nixie_segments = [
        nixie_a.downgrade(),
        nixie_b.downgrade(),
        nixie_c.downgrade(),
        nixie_d.downgrade(),
        nixie_e.downgrade(),
        nixie_f.downgrade(),
        nixie_g.downgrade(),
      ];

      let nixie_display = NixieDisplay::new(
        nixie1.downgrade(),
        nixie2.downgrade(),
        nixie_segments,
        nixie_dp.downgrade(),
        hv_enable.downgrade(),
      );
      let nixie_clock = NixieClock::new(
        nixie_display,
        200
      );
      *NIXIE_DISPLAY.borrow(cs).borrow_mut() = Some(nixie_clock);

      //setup i2c for the gyro
      let sda = gpiob.pb7.into_alternate_af1(cs);
      let scl = gpiob.pb6.into_alternate_af1(cs);

      let i2c = I2c::i2c1(p.I2C1, (scl, sda), 100.khz(), &mut rcc);
      let mut mpu = Mpu6050::new(i2c);
      let mut delay = Delay::new(cp.SYST, &rcc);
      mpu.init(&mut delay).unwrap();
      mpu.setup_motion_detection().unwrap();

      //Disable the gyroscope for 3mA less power usage :)
      mpu.write_bit(0x6C, 0, true).unwrap();
      mpu.write_bit(0x6C, 1, true).unwrap();
      mpu.write_bit(0x6C, 2, true).unwrap();
      *MPU.borrow(cs).borrow_mut() = Some(mpu);

      // Set up a timer expiring after 1s
      let mut timer = Timer::tim14(p.TIM14, Hertz(200), &mut rcc);
      // Generate an interrupt when the timer expires
      timer.listen(Event::TimeOut);

      // Init the ADC
      let adc = Adc::new(p.ADC, &mut rcc);
      let battery_voltage = gpioa.pa0.into_analog(cs);
      *ADC.borrow(cs).borrow_mut() = Some(adc);
      *BATTERY_VOLTAGE.borrow(cs).borrow_mut() = Some(battery_voltage);


      // Move the timer into our global storage
      *GINT.borrow(cs).borrow_mut() = Some(timer);

      *EINT.borrow(cs).borrow_mut() = Some(exti);

      // Charge pin move
      *CHARGE_STATUS.borrow(cs).borrow_mut() = Some(charge_status);


      // Enable TIM14 IRQ, set prio 1 and clear any pending IRQs
      let mut nvic = cp.NVIC;
      unsafe {
        nvic.set_priority(Interrupt::TIM14, 16);
        nvic.set_priority(Interrupt::USB, 32);
        nvic.set_priority(Interrupt::EXTI4_15, 1);
        cortex_m::peripheral::NVIC::unmask(Interrupt::TIM14);
        cortex_m::peripheral::NVIC::unmask(Interrupt::USB);
        cortex_m::peripheral::NVIC::unmask(Interrupt::EXTI4_15)
      }
      cortex_m::peripheral::NVIC::unpend(Interrupt::TIM14);
      cortex_m::peripheral::NVIC::unpend(Interrupt::USB);
      cortex_m::peripheral::NVIC::unpend(Interrupt::EXTI4_15);

      let usb = Peripheral {
        usb: p.USB,
        pin_dm: gpioa.pa11,
        pin_dp: gpioa.pa12,
      };

      let mut usb_serial = UsbSerial::new();
      usb_serial.init(usb);
      *USB_SERIAL.borrow(cs).borrow_mut() = Some(usb_serial);

    });
  }  

  loop {
    wfi();
  }
}
