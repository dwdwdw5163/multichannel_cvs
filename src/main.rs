use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use anyhow::Error;

use bytemuck::NoUninit;
use visa_rs::{enums::attribute::{AttrTermchar, AttrTermcharEn, AttrTmoValue, HasAttribute}, vs::{viPrintf, viSetAttribute, VI_TRUE}};

#[derive(Debug, Clone, Copy)]
struct State {
    /// channel number 0-255
    channel: u8,
    /// voltage -10-10V
    voltage: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct DataFormat {
    ///固定为 AABB
    head0: u8,
    head1: u8,
    ///Upper 4bit: 用于区分不同功能，电压配置为“0001” Lower 4bit:用于区分不同芯片，1-32 路芯片为“0000” 32-64 路芯片为“0001”
    fb_chip: u8,
    ///功能位 2bit 地址位 6bit b 001000- b 100111
    instrcution: u8,
    ///数据位 16bit
    data: u16,
    blank: u8,
    check: u8,
}

unsafe impl NoUninit for DataFormat {}

impl Default for DataFormat {
    fn default() -> Self {
        DataFormat {
            head0: 0xAA,
            head1: 0xBB,
            fb_chip: 0x10,
            instrcution: 0xC8,
            data: 0x0000,
            blank: 0x00,
            check: 0x00,
        }
    }
}

impl From<State> for DataFormat {
    fn from(value: State) -> Self {
        let mut data_format = DataFormat::default();

        // chip select,
        data_format.fb_chip += value.channel / 32;
        // channel select
        data_format.instrcution += value.channel % 32;
        // data voltage/max voltage * 2^14
        let data: f32 = (value.voltage + 10.0) / 20.0 * 2.0_f32.powi(16);
        let data = data as u16;

        data_format.data = data.swap_bytes(); // format to big endian
        
        let checksum: u8 =
            0xaa_u8
                .wrapping_add(0xbb_u8)
                .wrapping_add(data_format.fb_chip)
                .wrapping_add(data_format.instrcution)
                .wrapping_add((data_format.data >> 8) as u8)
                .wrapping_add((data_format.data & 0xff) as u8);

        data_format.check = !checksum;

        data_format
    }
}


fn find_an_instr() -> visa_rs::Result<()>{
    use std::ffi::CString;
    use std::io::{BufRead, BufReader, Read, Write};
    use visa_rs::prelude::*;

    // open default resource manager
    let rm: DefaultRM = DefaultRM::new()?;

    // expression to match resource name
    let expr = CString::new("USB0::0x1313?*INSTR").unwrap().into();

    // find the first resource matched
    let rsc = rm.find_res(&expr)?;
    
    // open a session to the resource, the session will be closed when rm is dropped
    let instr: Instrument = rm.open(&rsc, AccessMode::NO_LOCK, TIMEOUT_IMMEDIATE)?;

    // write message
    (&instr).write_all(b"*IDN?\n").map_err(io_to_vs_err)?;

    // read response
    let mut buf_reader = BufReader::new(&instr);
    let mut buf = String::new();
    buf_reader.read_line(&mut buf).map_err(io_to_vs_err)?;

    eprintln!("{}", buf);

    //Read the power

    for _ in 0..10 {
        (&instr).write_all(b"MEASURE:POWER?\n").map_err(io_to_vs_err)?;
        buf_reader.read_line(&mut buf).map_err(io_to_vs_err)?;
	println!("{}", buf);
    }

    
    
    //Disconnect the device
    instr.clear();
    rm.close_all();
    
    
    Ok(())
}

fn main() -> Result<(), Error>{

    find_an_instr()?;
    
    let state = State {channel:2, voltage: 0.0};
    let data = DataFormat::from(state);

    let socket = UdpSocket::bind("169.254.1.205:1234").expect("couldn't bind to address");
    let receiver_addr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(169, 254, 1, 10)), 1234);
    
    println!("{:X?}", bytemuck::bytes_of(&data));

    socket.send_to(bytemuck::bytes_of(&data), receiver_addr).expect("couldn't send data");

    Ok(())
}
