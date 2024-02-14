use soroban_sdk::{Address, Vec};

pub fn check_vec_ordered(addresses: &Vec<Address>) -> bool {
    for i in 0..addresses.len() - 1 {
        if addresses.get(i).unwrap() > addresses.get(i + 1).unwrap() {
            return false;
        }
    }
    true
}
