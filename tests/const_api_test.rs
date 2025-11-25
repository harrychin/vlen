
use vlen::const_encode::{encode_u32, encode_i64};
use vlen::const_decode::{decode_u32, decode_i64};

const fn test_const_encode_decode() -> bool {
    // Test u32
    let mut buf_u32 = [0u8; 5];
    let len_u32 = encode_u32(&mut buf_u32, 12345);
    let (val_u32, len_decoded_u32) = decode_u32(&buf_u32);
    
    if len_u32 != len_decoded_u32 { return false; }
    if val_u32 != 12345 { return false; }

    // Test i64
    let mut buf_i64 = [0u8; 9];
    let len_i64 = encode_i64(&mut buf_i64, -1234567890);
    let (val_i64, len_decoded_i64) = decode_i64(&buf_i64);

    if len_i64 != len_decoded_i64 { return false; }
    if val_i64 != -1234567890 { return false; }

    true
}

const TEST_RESULT: bool = test_const_encode_decode();

#[test]
fn test_const_works() {
    assert!(TEST_RESULT);
}
