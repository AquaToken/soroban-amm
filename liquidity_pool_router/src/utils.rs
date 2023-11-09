use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};

pub fn sort(a: &Address, b: &Address) -> (Address, Address) {
    if a < b {
        return (a.clone(), b.clone());
    } else if a > b {
        return (b.clone(), a.clone());
    }
    panic!("a and b can't be the same")
}

pub fn pool_salt(e: &Env, tokens: Vec<Address>) -> BytesN<32> {
    for i in 0..tokens.len() - 1 {
        if tokens.get_unchecked(i) >= tokens.get_unchecked(i + 1) {
            panic!("tokens must be sorted by ascending");
        }
    }

    let mut salt = Bytes::new(e);
    for token in tokens.into_iter() {
        salt.append(&token.to_xdr(e));
    }
    e.crypto().sha256(&salt)
}
