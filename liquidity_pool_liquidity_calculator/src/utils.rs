// use num_bigint::BigUint;

// pub fn biguint_to_128(value: BigUint) -> u128 {
//     let be_bytes = value.to_bytes_be();
//     let mut result: [u8; 16] = [0; 16];
//
//     if be_bytes.len() > 16 {
//         panic!("value overflow");
//     }
//
//     for i in 0..be_bytes.len() {
//         result[16 - i - 1] = be_bytes[be_bytes.len() - 1 - i];
//     }
//     u128::from_be_bytes(result)
// }
