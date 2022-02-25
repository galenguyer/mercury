#![allow(dead_code)]
use embedded_hal::{
    blocking::delay::*,
    digital::v2::{InputPin, OutputPin},
};
use esp_idf_hal::{delay::Ets, interrupt::CriticalSection};

#[derive(Debug)]
pub struct DHT22<P> {
    pin: P,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Error<I>(ErrorKind<I>);

#[derive(Eq, PartialEq, Debug)]
enum ErrorKind<I> {
    Io(I),
    Checksum { actual: u8, expected: u8 },
    Timeout,
}

#[derive(Debug, Clone)]
pub struct Reading {
    rh_integral: u8,
    rh_decimal: u8,
    t_integral: u8,
    t_decimal: u8,
}

#[derive(Copy, Clone, Debug)]
pub struct Pulse {
    lo: u8,
    hi: u8,
}

impl<P, E> DHT22<P>
where
    P: InputPin<Error = E> + OutputPin<Error = E>,
{
    pub fn new(pin: P) -> Self {
        Self { pin }
    }
}

impl<P, E> DHT22<P>
where
    P: InputPin<Error = E> + OutputPin<Error = E>,
{
    #[inline(always)] // timing-critical
    fn read_pulse_us(&mut self, high: bool) -> Result<u8, ErrorKind<E>> {
        for len in 0..=core::u8::MAX {
            if self.pin.is_high()? != high {
                return Ok(len);
            }
            Ets.delay_us(1_u32);
        }
        // Return an Error instead
        Err(ErrorKind::Timeout)
    }

    #[inline(always)] // timing-critical
    fn start_signal_blocking(&mut self) -> Result<(), ErrorKind<E>> {
        self.pin.set_high()?;
        Ets.delay_ms(1_u32);

        self.pin.set_low()?;
        Ets.delay_us(1200_u32);
        self.pin.set_high()?;
        Ets.delay_us(40_u32);

        self.read_pulse_us(false)?;
        self.read_pulse_us(true)?;

        Ok(())
    }

    #[inline(always)] // timing-critical
    pub fn read_blocking(&mut self) -> Result<Reading, Error<E>> {
        self.start_signal_blocking().map_err(ErrorKind::from)?;

        // The sensor will now send us 40 bits of data. For each bit, the sensor
        // will assert the line low for 50 microseconds as a delimiter, and then
        // will assert the line high for a variable-length pulse to encode the
        // bit. If the high pulse is 70 us long, then the bit is 1, and if it is
        // 28 us, then the bit is a 0.
        //
        // Because timing is sloppy, we will read each bit by comparing the
        // length of the initial low pulse with the length of the following high
        // pulse. If it was longer than the 50us low pulse, then it's closer to
        // 70us, and if it was shorter, than it is closer to 28 us.
        let mut pulses = [Pulse { lo: 0, hi: 0 }; 40];

        // Read each bit from the sensor now. We'll convert the raw pulses into
        // bytes in a subsequent step, to avoid doing that work in the
        // timing-critical loop.

        let cs = CriticalSection::new();
        let csg = cs.enter();
        for pulse in &mut pulses[..] {
            pulse.lo = self.read_pulse_us(false)?;
            pulse.hi = self.read_pulse_us(true)?;
        }
        drop(csg);
        Ok(Reading::from_pulses(&pulses)?)
    }
}

impl Reading {
    fn from_pulses<E>(pulses: &[Pulse; 40]) -> Result<Self, ErrorKind<E>> {
        let mut bytes = [0u8; 5];
        // The last byte sent by the sensor is a checksum, which should be the
        // low byte of the 16-bit sum of the first four data bytes.
        let mut chksum: u16 = 0;
        for (i, pulses) in pulses.chunks(8).enumerate() {
            let byte = &mut bytes[i];
            // If the high pulse is longer than the leading low pulse, the bit
            // is a 1, otherwise, it's a 0.
            for Pulse { lo, hi } in pulses {
                *byte <<= 1;
                if hi > lo {
                    *byte |= 1;
                }
            }
            // If this isn't the last byte, then add it to the checksum.
            if i < 4 {
                chksum += *byte as u16;
            }
        }

        // Does the checksum match?
        let expected = bytes[4];
        let actual = chksum as u8;
        if actual != expected {
            return Err(ErrorKind::Checksum { actual, expected });
        }

        Ok(Self {
            rh_integral: bytes[0],
            rh_decimal: bytes[1],
            t_integral: bytes[2],
            t_decimal: bytes[3],
        })
    }

    /// Returns the temperature in Fahrenheit.
    pub fn temp_fahrenheit(self) -> f32 {
        celcius_to_fahrenheit(self.temp_celcius())
    }

    pub fn temp_celcius(self) -> f32 {
        let mut temp = (((self.t_integral & 0x7F) as u16) << 8 | self.t_decimal as u16) as f32;
        temp *= 0.1;
        if self.t_integral & 0x80 != 0 {
            temp *= -1.0;
        }
        temp
    }

    pub fn humidity_percent(self) -> f32 {
        ((self.rh_integral as u16) << 8 | self.rh_decimal as u16) as f32 * 0.1
    }
}

impl<E> From<E> for ErrorKind<E> {
    fn from(e: E) -> Self {
        ErrorKind::Io(e)
    }
}

impl<E> From<ErrorKind<E>> for Error<E> {
    fn from(e: ErrorKind<E>) -> Self {
        Self(e)
    }
}

impl<E> Error<E> {
    pub fn is_timeout(&self) -> bool {
        matches!(self.0, ErrorKind::Timeout)
    }

    pub fn is_io(&self) -> bool {
        matches!(self.0, ErrorKind::Io(_))
    }

    pub fn is_checksum(&self) -> bool {
        matches!(self.0, ErrorKind::Checksum{ .. })
    }

    pub fn into_io(self) -> Option<E> {
        match self.0 {
            ErrorKind::Io(io) => Some(io),
            _ => None,
        }
    }
}

fn celcius_to_fahrenheit(c: f32) -> f32 {
    c * 1.8 + 32.0
}
