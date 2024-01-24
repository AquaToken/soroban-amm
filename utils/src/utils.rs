use soroban_sdk::{Address, Vec};

pub fn sort(a: &Address, b: &Address) -> (Address, Address) {
    if a == b {
        panic!("a and b can't be the same")
    }
    match a < b {
        true => (a.clone(), b.clone()),
        false => (b.clone(), a.clone()),
    }
}

pub fn check_vec_ordered(addresses: &Vec<Address>) -> bool {
    for i in 0..addresses.len() - 1 {
        if addresses.get(i).unwrap() > addresses.get(i + 1).unwrap() {
            return false;
        }
    }
    true
}
