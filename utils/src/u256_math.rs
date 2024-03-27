use soroban_sdk::U256;

pub trait ExtraMath {
    fn sqrt(&self) -> Self;
}

impl ExtraMath for U256 {
    fn sqrt(&self) -> U256 {
        // https://github.com/paritytech/parity-common/issues/252
        let e = self.env();
        let two = U256::from_u32(e, 2);

        let mut z = (self.add(&U256::from_u32(e, 1))).div(&two);

        let mut y = self.clone();

        while z < y {
            y = z.clone();
            z = (self.div(&z).add(&z)).div(&two);
        }

        y
    }
}
