//! I2C interface to the HTS221.
//!
//! The API is modeled roughly after device crates generated by `svd2rust`, though not quite as
//! type-driven (for example, there are no R or W types).  This module provides access to every
//! register (or set of related registers) defined in the
//! [datasheet](http://www.st.com/resource/en/datasheet/hts221.pdf).

/// 7-bit I2C slave address of the HTS221.  Note that the datasheet includes the 8-bit read address
/// (BFh) and 8-bit write address (BEh).
const I2C_ID: u8 = 0x5F;

/// I2C defines blocking functions that write and read data on the I2C bus.  All device
/// communication goes through a type that implements I2C.
pub trait I2C {
    type Error;

    /// Blocking write to I2C slave.  Sends `bytes` to slave with address `dev_addr`.
    fn write(&mut self, dev_addr: u8, bytes: &[u8]) -> Result<(), Self::Error>;

    /// Blocking write to, followed by blocking read from, I2C slave.  Sends `bytes` to slave with
    /// address `dev_addr` and then reads enough bytes to fill `buffer` *in a single transaction*.
    fn write_read(
        &mut self,
        dev_addr: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> Result<(), Self::Error>;
}

fn read_register<Comm: I2C>(comm: &mut Comm, addr: u8) -> Result<u8, Comm::Error> {
    let mut data: [u8; 1] = [0];
    comm.write_read(I2C_ID, &[addr], &mut data)?;
    Ok(data[0])
}

fn write_register<Comm: I2C>(comm: &mut Comm, addr: u8, bits: u8) -> Result<(), Comm::Error> {
    comm.write(I2C_ID, &[addr, bits])
}

fn read_register_pair<Comm: I2C>(comm: &mut Comm, addr: u8) -> Result<i16, Comm::Error> {
    let mut data: [u8; 2] = [0; 2];
    comm.write_read(I2C_ID, &[addr], &mut data)?;
    Ok(((data[1] as i16) << 8) | (data[0] as i16))
}

/// The WHO_AM_I register, for device identification.
pub struct WhoAmI(u8);

/// Constants for WHO_AM_I.
pub mod who_am_i {
    /// Sub-address of the register.
    pub const ADDR: u8 = 0x0F;
}

impl WhoAmI {
    /// Blocking read of the WHO_AM_I register over `comm`.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let bits = read_register(comm, who_am_i::ADDR)?;
        Ok(WhoAmI(bits))
    }

    /// Returns the device ID, which should be 0xBC.
    pub fn device_id(&self) -> u8 {
        self.0
    }
}

/// The AV_CONF register.  Controls humidity and temperature resolution modes.
pub struct AvConf(u8);

/// Constants for AV_CONF.
pub mod av_conf {
    /// Sub-address of the register.
    pub const ADDR: u8 = 0x10;

    /// The humidity configuration is 3 bits.
    pub const H_MASK: u8 = 0b111;

    /// The humidity configuration bits start at bit 0.
    pub const H_OFFSET: u8 = 0;

    /// Values for humidity configuration.  Selects the number of internal humidity samples averaged
    /// into one sample.
    #[repr(u8)]
    pub enum AvgH {
        Avg4 = 0,
        Avg8 = 1,
        Avg16 = 2,
        Avg32 = 3,
        Avg64 = 4,
        Avg128 = 5,
        Avg256 = 6,
        Avg512 = 7,
    }

    /// The temperature configuration is 3 bits.
    pub const T_MASK: u8 = 0b111;

    /// The temperature configuration bits start at bit 3.
    pub const T_OFFSET: u8 = 3;

    /// Values for temperature configuration.  Selects the number of internal temperature samples
    /// averaged into one sample.
    #[repr(u8)]
    pub enum AvgT {
        Avg2 = 0,
        Avg4 = 1,
        Avg8 = 2,
        Avg16 = 3,
        Avg32 = 4,
        Avg64 = 5,
        Avg128 = 6,
        Avg256 = 7,
    }
}
impl AvConf {
    /// Blocking read of the AV_CONF register over `comm`.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let bits = read_register(comm, av_conf::ADDR)?;
        Ok(AvConf(bits))
    }

    /// Updates the register using `f`, then writes the new value out to the chip.
    pub fn modify<Comm, F>(&mut self, comm: &mut Comm, f: F) -> Result<(), Comm::Error>
    where
        Comm: I2C,
        F: FnOnce(&mut Self),
    {
        f(self);
        write_register(comm, av_conf::ADDR, self.0)
    }

    /// Returns the number of internal humidity samples averaged together to generate one sample.
    /// Note that this is an interpretation of the bit pattern, not the bit pattern itself.
    pub fn humidity_samples_averaged(&self) -> u16 {
        match (self.0 >> av_conf::H_OFFSET) & av_conf::H_MASK {
            0 => 4,   // av_conf::AvgH::Avg4,
            1 => 8,   // av_conf::AvgH::Avg8,
            2 => 16,  // av_conf::AvgH::Avg16,
            3 => 32,  // av_conf::AvgH::Avg32,
            4 => 64,  // av_conf::AvgH::Avg64,
            5 => 128, // av_conf::AvgH::Avg128,
            6 => 256, // av_conf::AvgH::Avg256,
            7 => 512, // av_conf::AvgH::Avg512,
            _ => panic!("Unreachable"),
        }
    }

    /// Sets the number of internal humidity samples that are averaged together to generate one
    /// sample.  Use inside a `modify` function to actually set the value on the chip.
    ///
    /// Do this:
    /// ```
    /// let av_conf = hts221.av_conf()?;
    /// av_conf.modify(|w| w.set_humidity_samples_averaged(AvgH::Avg8))?;
    /// ```
    ///
    /// Instead of this:
    /// ```
    /// let av_conf = hts221.av_conf()?;
    /// av_conf.set_humidity_samples_averaged(AvgH::Avg8)?;  // not written to chip
    /// ```
    pub fn set_humidity_samples_averaged(&mut self, samples: av_conf::AvgH) {
        self.0 &= !(av_conf::H_MASK << av_conf::H_OFFSET);
        self.0 |= (samples as u8) << av_conf::H_OFFSET;
    }

    /// Returns the number of internal temperature samples averaged together to generate one sample.
    /// Note that this is an interpretation of the bit pattern, not the bit pattern itself.
    pub fn temperature_samples_averaged(&self) -> u16 {
        match (self.0 >> av_conf::T_OFFSET) & av_conf::T_MASK {
            0 => 2,   // av_conf::AvgT::Avg2,
            1 => 4,   // av_conf::AvgT::Avg4,
            2 => 8,   // av_conf::AvgT::Avg8,
            3 => 16,  // av_conf::AvgT::Avg16,
            4 => 32,  // av_conf::AvgT::Avg32,
            5 => 64,  // av_conf::AvgT::Avg64,
            6 => 128, // av_conf::AvgT::Avg128,
            7 => 256, // av_conf::AvgT::Avg256,
            _ => panic!("Unreachable"),
        }
    }

    /// Sets the number of internal temperature samples that are averaged together to generate one
    /// sample.  Use inside a `modify` function to actually set the value on the chip.
    pub fn set_temperature_samples_averaged(&mut self, samples: av_conf::AvgT) {
        self.0 &= !(av_conf::T_MASK << av_conf::T_OFFSET);
        self.0 |= (samples as u8) << av_conf::T_OFFSET;
    }
}

/// The CTRL_REG1 register.  Contains power on, data transfer mode, and data rate configuration.
pub struct CtrlReg1(u8);

/// Constants for CTRL_REG1.
pub mod cr1 {
    /// Sub-address of the register.
    pub const ADDR: u8 = 0x20;

    /// The power-down bit is bit 7.
    pub const PD_BIT: u8 = 7;

    /// The block data update bit is bit 2.
    pub const BDU_BIT: u8 = 2;

    /// The output data rate configuration is 2 bits.
    pub const ODR_MASK: u8 = 0b11;

    /// The output data rate configuration bits start at bit 0.
    pub const ODR_OFFSET: u8 = 0;

    /// Values of the output data rate.
    #[repr(u8)]
    pub enum DataRate {
        OneShot = 0b00,
        Continuous1Hz = 0b01,
        Continuous7Hz = 0b10,
        Continuous12_5Hz = 0b11,
    }
}
impl CtrlReg1 {
    /// Blocking read of the CTRL_REG1 register over `comm`.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let bits = read_register(comm, cr1::ADDR)?;
        Ok(CtrlReg1(bits))
    }

    /// Updates the register using `f`, then writes the new value out to the chip.
    pub fn modify<Comm, F>(&mut self, comm: &mut Comm, f: F) -> Result<(), Comm::Error>
    where
        Comm: I2C,
        F: FnOnce(&mut Self),
    {
        f(self);
        write_register(comm, cr1::ADDR, self.0)
    }

    /// Returns true if the chip is active.
    pub fn is_powered_up(&self) -> bool {
        (self.0 & cr1::PD_BIT) > 0
    }

    /// Clears the power-down bit.  The device is in power-down mode when PD = 0.
    pub fn power_down(&mut self) {
        self.0 &= !(1 << cr1::PD_BIT);
    }

    /// Sets the power-down bit.  The device is active when PD = 1.
    pub fn power_up(&mut self) {
        self.0 |= 1 << cr1::PD_BIT;
    }

    /// Returns true if the chip is using block-update mode.
    pub fn is_block_update(&self) -> bool {
        (self.0 & cr1::BDU_BIT) > 0
    }

    /// Clears the block-update mode bit.  In default (continuous) mode, the lower and upper parts
    /// of the output registers are updated continuously. If it is not certain whether the read will
    /// be faster than output data rate, it is recommended to use block-update mode.
    pub fn set_continuous_update(&mut self) {
        self.0 &= !(1 << cr1::BDU_BIT);
    }

    /// Sets the block-update mode bit.  In block-update mode, after the reading of the lower
    /// (upper) register part, the content of that output register is not updated until the upper
    /// (lower) part is read also.  This feature prevents the reading of LSB and MSB related to
    /// different samples.
    pub fn set_block_update(&mut self) {
        self.0 |= 1 << cr1::BDU_BIT;
    }

    /// Returns the configured data rate.
    pub fn data_rate(&self) -> cr1::DataRate {
        match (self.0 >> cr1::ODR_OFFSET) & cr1::ODR_MASK {
            0b00 => cr1::DataRate::OneShot,
            0b01 => cr1::DataRate::Continuous1Hz,
            0b10 => cr1::DataRate::Continuous7Hz,
            0b11 => cr1::DataRate::Continuous12_5Hz,
            _ => panic!("unreachable"),
        }
    }

    /// Sets the output data rates of humidity and temperature samples.
    pub fn set_data_rate(&mut self, rate: cr1::DataRate) {
        self.0 &= !(cr1::ODR_MASK << cr1::ODR_OFFSET);
        self.0 |= (rate as u8) << cr1::ODR_OFFSET;
    }
}

/// The CTRL_REG2 register.
pub struct CtrlReg2(u8);

/// Constants for CTRL_REG2.
pub mod cr2 {
    /// Sub-address of the register.
    pub const ADDR: u8 = 0x21;

    /// The boot bit is bit 7.
    pub const BOOT_BIT: u8 = 7;

    /// The heater bit is bit 1.
    pub const HEATER_BIT: u8 = 1;

    /// The one-shot bit is bit 0.
    pub const ONE_SHOT_BIT: u8 = 0;
}
impl CtrlReg2 {
    /// Blocking read of the CTRL_REG2 register over `comm`.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let bits = read_register(comm, cr2::ADDR)?;
        Ok(CtrlReg2(bits))
    }

    /// Updates the register using `f`, then writes the new value out to the chip.
    pub fn modify<Comm, F>(&mut self, comm: &mut Comm, f: F) -> Result<(), Comm::Error>
    where
        Comm: I2C,
        F: FnOnce(&mut Self),
    {
        f(self);
        write_register(comm, cr2::ADDR, self.0)
    }

    /// Returns true if the chip is booting.
    pub fn is_booting(&self) -> bool {
        (self.0 & cr2::BOOT_BIT) > 0
    }

    /// Sets the boot bit.  From the datasheet:
    ///
    /// > The BOOT bit is used to refresh the content of the internal registers stored in the Flash
    /// > memory block. At device power-up, the content of the Flash memory block is transferred to
    /// > the internal registers related to trimming functions to permit good behavior of the device
    /// > itself. If, for any reason, the content of the trimming registers is modified, it is
    /// > sufficient to use this bit to restore the correct values. When the BOOT bit is set to ‘1’
    /// > the content of the internal Flash is copied inside the corresponding internal registers
    /// > and is used to calibrate the device. These values are factory trimmed and are different
    /// > for every device. They permit good behavior of the device and normally they should not be
    /// > changed. At the end of the boot process, the BOOT bit is set again to ‘0’.
    pub fn boot(&mut self) {
        self.0 |= 1 << cr2::BOOT_BIT;
    }

    /// Returns true if the heating element is on.
    pub fn is_heater_on(&self) -> bool {
        (self.0 & cr2::HEATER_BIT) > 0
    }

    /// Enables the heating element.
    pub fn set_heater_on(&mut self) {
        self.0 |= 1 << cr2::HEATER_BIT;
    }

    /// Disables the heating element.
    pub fn set_heater_off(&mut self) {
        self.0 &= !(1 << cr2::HEATER_BIT);
    }

    /// Returns true if a one-shot conversion is pending.
    pub fn is_one_shot(&self) -> bool {
        (self.0 & cr2::ONE_SHOT_BIT) > 0
    }

    /// Initiates a one-shot conversion.  The bit will be cleared by hardware after the conversion
    /// is complete.
    pub fn set_one_shot(&mut self) {
        self.0 |= 1 << cr2::ONE_SHOT_BIT;
    }
}

/// The CTRL_REG3 register.
pub struct CtrlReg3(u8);

/// Constants for CTRL_REG3.
pub mod cr3 {
    /// Sub-address of the register.
    pub const ADDR: u8 = 0x22;

    /// The data ready polarity bit is bit 7.
    pub const DRDY_H_L_BIT: u8 = 7;

    /// The data ready mode bit is bit 6.
    pub const PP_OD_BIT: u8 = 6;

    /// The bit to enable an interrupt on data ready is bit 2.
    pub const DRDY_BIT: u8 = 2;
}
impl CtrlReg3 {
    /// Blocking read of the CTRL_REG3 register over `comm`.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let bits = read_register(comm, cr3::ADDR)?;
        Ok(CtrlReg3(bits))
    }

    /// Updates the register using `f`, then writes the new value out to the chip.
    pub fn modify<Comm, F>(&mut self, comm: &mut Comm, f: F) -> Result<(), Comm::Error>
    where
        Comm: I2C,
        F: FnOnce(&mut Self),
    {
        f(self);
        write_register(comm, cr3::ADDR, self.0)
    }

    /// Sets the data ready output signal to ready = high, not ready = low.
    pub fn data_ready_high(&mut self) {
        self.0 &= !(1 << cr3::DRDY_H_L_BIT);
    }

    /// Sets the data ready output signal to ready = low, not ready = high.
    pub fn data_ready_low(&mut self) {
        self.0 |= 1 << cr3::DRDY_H_L_BIT;
    }

    /// Sets the data ready output pin to push/pull mode.
    pub fn data_ready_push_pull(&mut self) {
        self.0 &= !(1 << cr3::PP_OD_BIT);
    }

    /// Sets the data ready output pin open drain.
    pub fn data_ready_open_drain(&mut self) {
        self.0 |= 1 << cr3::PP_OD_BIT;
    }

    /// Disables the data ready signal on pin 3.
    pub fn data_ready_disable(&mut self) {
        self.0 &= !(1 << cr3::DRDY_BIT);
    }

    /// Enables the data ready signal on pin 3.
    pub fn data_ready_enable(&mut self) {
        self.0 |= 1 << cr3::DRDY_BIT;
    }
}

/// The STATUS register.
pub struct StatusReg(u8);

/// Constants for STATUS.
pub mod status {
    /// Sub-address of the register.
    pub const ADDR: u8 = 0x27;

    /// The humidity ready bit is bit 1.
    pub const HUMIDITY_BIT: u8 = 1;

    /// The temperature ready bit is bit 0.
    pub const TEMPERATURE_BIT: u8 = 0;
}
impl StatusReg {
    /// Blocking read of STATUS over `comm`.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let bits = read_register(comm, status::ADDR)?;
        Ok(StatusReg(bits))
    }

    /// Returns true if humidity data is available.
    pub fn humidity_data_available(&self) -> bool {
        self.0 & (1 << status::HUMIDITY_BIT) > 0
    }

    /// Returns true if temperature data is available.
    pub fn temperature_data_available(&self) -> bool {
        self.0 & (1 << status::TEMPERATURE_BIT) > 0
    }
}

/// Combination of HUMIDITY_OUT_L and HUMIDITY_OUT_H registers.
pub struct HumidityOut(i16);

/// Constants for HUMIDITY_OUT_L and HUMIDITY_OUT_H.
pub mod h_out {
    /// Sub-address of the registers.  HUMIDITY_OUT_L address is 0x28, HUMIDITY_OUT_H is 0x29, but
    /// we set the high bit to read both in the same transfer.
    pub const ADDR: u8 = 0x80 | 0x28;
}
impl HumidityOut {
    /// Blocking read of both registers over `comm`.  Stores the signed 16-bit value created from
    /// combining the registers.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let bits = read_register_pair(comm, h_out::ADDR)?;
        Ok(HumidityOut(bits))
    }

    /// Returns the raw humidity sample value.
    pub fn value(&self) -> i16 {
        self.0
    }
}

/// Combination of TEMP_OUT_L and TEMP_OUT_H registers.
pub struct TemperatureOut(i16);

/// Constants for TEMP_OUT_L and TEMP_OUT_H.
pub mod t_out {
    /// Sub-address of the registers.  TEMP_OUT_L address is 0x2A, TEMP_OUT_H is 0x2B, but we set
    /// the high bit to read both in the same transfer.
    pub const ADDR: u8 = 0x80 | 0x2A;
}
impl TemperatureOut {
    /// Blocking read of both registers over `comm`.  Stores the signed 16-bit value created from
    /// combining the registers.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let bits = read_register_pair(comm, t_out::ADDR)?;
        Ok(TemperatureOut(bits))
    }

    /// Returns the raw temperature sample value.
    pub fn value(&self) -> i16 {
        self.0
    }
}

/// Calibration data for the particular chip.  All chips are factory-calibrated, and require no
/// further calibration from the user.
pub struct Calibration {
    /// Relative humidity from calibration point 0.
    pub h0_rh_x2: u8,
    /// Relative humidity from calibration point 1.
    pub h1_rh_x2: u8,
    /// Temperature from calibration point 0.
    pub t0_deg_c_x8: u16,
    /// Temperature from calibration point 1.
    pub t1_deg_c_x8: u16,
    /// Humidity ADC reading from calibration point 0.
    pub h0_t0_out: i16,
    /// Humidity ADC reading from calibration point 1.
    pub h1_t0_out: i16,
    /// Temperature ADC reading from calibration point 0.
    pub t0_out: i16,
    /// Temperature ADC reading from calibration point 1.
    pub t1_out: i16,
}
pub mod calibration {
    /// Sub-address of the registers.  The calibration registers start at 0x30. By setting the high
    /// bit, we can read all registers in the same transfer.
    pub const ADDR: u8 = 0x80 | 0x30;
}
impl Calibration {
    /// Blocking read of the calibration registers over `comm`.
    pub fn new<Comm>(comm: &mut Comm) -> Result<Self, Comm::Error>
    where
        Comm: I2C,
    {
        let mut data: [u8; 16] = [0; 16];
        comm.write_read(I2C_ID, &[calibration::ADDR], &mut data)?;
        Ok(Calibration {
            h0_rh_x2: data[0],
            h1_rh_x2: data[1],
            t0_deg_c_x8: ((((data[5] & 0b11) as u16) << 8) | data[2] as u16),
            t1_deg_c_x8: (((((data[5] & 0b1100) >> 2) as u16) << 8) | data[3] as u16),
            h0_t0_out: (data[7] as i16) << 8 | data[6] as i16,
            h1_t0_out: (data[11] as i16) << 8 | data[10] as i16,
            t0_out: (data[13] as i16) << 8 | data[12] as i16,
            t1_out: (data[15] as i16) << 8 | data[14] as i16,
        })
    }
}
