use core::{ops, cmp};
use soroban_sdk::{Env, U256};

#[derive(Clone)]
pub struct U256M {
    pub v: U256
}

impl U256M {
    pub fn from_u32(env: &Env, u: u32) -> Self {
        Self{v: U256::from_u32(env, u)}
    }
    pub fn from_u128(env: &Env, u: u128) -> Self {
        Self{v: U256::from_u128(env, u)}
    }
    pub fn from_u256(e: &Env, u: U256) -> Self {
        Self{v: u}
    }

    pub fn to_u128(&self) -> Option<u128> {
        self.v.to_u128()
    }

    pub fn pow(&self, pow: u32) -> Self {
        return Self{v: self.v.pow(pow)}
    }
}

impl ops::Add<U256M> for U256M {
    type Output = U256M;

    fn add(self, rhs: U256M) -> U256M {
        U256M{v: self.v.add(&rhs.v)}
    }
}

impl ops::Add<&U256M> for &U256M {
    type Output = U256M;

    fn add(self, rhs: &U256M) -> U256M {
        U256M{v: self.v.add(&rhs.v)}
    }
}

impl ops::Add<&U256M> for U256M {
    type Output = U256M;

    fn add(self, rhs: &U256M) -> U256M {
        U256M{v: self.v.add(&rhs.v)}
    }
}

impl ops::Sub<U256M> for U256M {
    type Output = U256M;

    fn sub(self, rhs: U256M) -> U256M {
        U256M{v: self.v.sub(&rhs.v)}
    }
}

impl ops::Sub<&U256M> for &U256M {
    type Output = U256M;

    fn sub(self, rhs: &U256M) -> U256M {
        U256M{v: self.v.sub(&rhs.v)}
    }
}

impl ops::Sub<&U256M> for U256M {
    type Output = U256M;

    fn sub(self, rhs: &U256M) -> U256M {
        U256M{v: self.v.sub(&rhs.v)}
    }
}

impl ops::Sub<U256M> for &U256M {
    type Output = U256M;

    fn sub(self, rhs: U256M) -> U256M {
        U256M{v: self.v.sub(&rhs.v)}
    }
}

impl ops::Mul<U256M> for U256M {
    type Output = U256M;

    fn mul(self, rhs: U256M) -> U256M {
        U256M{v: self.v.mul(&rhs.v)}
    }
}

impl ops::Mul<&U256M> for U256M {
    type Output = U256M;

    fn mul(self, rhs: &U256M) -> U256M {
        U256M{v: self.v.mul(&rhs.v)}
    }
}

impl ops::Mul<&U256M> for &U256M {
    type Output = U256M;

    fn mul(self, rhs: &U256M) -> U256M {
        U256M{v: self.v.mul(&rhs.v)}
    }
}

impl ops::Mul<U256M> for &U256M {
    type Output = U256M;

    fn mul(self, rhs: U256M) -> U256M {
        U256M{v: self.v.mul(&rhs.v)}
    }
}

impl ops::Div<U256M> for U256M {
    type Output = U256M;

    fn div(self, rhs: U256M) -> U256M {
        U256M{v: self.v.div(&rhs.v)}
    }
}

impl ops::Div<&U256M> for &U256M {
    type Output = U256M;

    fn div(self, rhs: &U256M) -> U256M {
        U256M{v: self.v.div(&rhs.v)}
    }
}

impl ops::Div<&U256M> for U256M {
    type Output = U256M;

    fn div(self, rhs: &U256M) -> U256M {
        U256M{v: self.v.div(&rhs.v)}
    }
}

impl ops::Div<U256M> for &U256M {
    type Output = U256M;

    fn div(self, rhs: U256M) -> U256M {
        U256M{v: self.v.div(&rhs.v)}
    }
}
