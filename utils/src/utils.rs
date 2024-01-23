use soroban_sdk::{Address, Vec};

pub fn sort(a: &Address, b: &Address) -> (Address, Address) {
    if a < b {
        return (a.clone(), b.clone());
    } else if a > b {
        return (b.clone(), a.clone());
    }
    panic!("a and b can't be the same")
}

pub fn check_vec_ordered(addresses: &Vec<Address>) -> bool {
    for i in 0..addresses.len() - 1 {
        if addresses.get(i).unwrap() > addresses.get(i + 1).unwrap() {
            return false;
        }
    }
    true
}
