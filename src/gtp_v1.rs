use std::borrow::Borrow;

pub struct GtpV1 {
    length: u16,
    data: Vec<u8>
}

impl GtpV1 {
    pub fn init(data: Vec<u8>) -> GtpV1 {
        let p = GtpV1 {
            length: data.len() as u16,
            data
        };

        p
    }

    pub fn from_gtp(data: &[u8]) -> GtpV1 {
        let (header, _data) = data.split_at(8);
        let data = _data.to_vec();
        let p = GtpV1 {
            length: data.len() as u16,
            data
        };

        p
    }

    pub fn serialize(self: &mut GtpV1) -> Vec<u8> {
        // init data
        let mut header = [u8::from(0); 8];
        let data: &[u8] = self.data.borrow();

        // set header
        header[0] = 0x30;  // flags, 0011 0000
        header[1] = 0xff;  // message type, 1111 1111, T-PDU
        let length = [(self.length >> 8) as u8, self.length as u8];
        header[2] = length[0];  // length, lower bytes
        header[3] = length[1];  // length, upper bytes
        header[4] = 0x00;  // teid byte 1
        header[5] = 0x00;  // teid byte 2
        header[6] = 0x00;  // teid byte 3
        header[7] = 0x01;  // teid byte 4

        let packet: Vec<u8> = [header.as_ref(), data].concat();

        packet
    }

}
