use dep::std;

pub fn prefix_sha256_hash<N>(prefix_length: u8, input: [u8; N]) -> [u8; 32] {
    let mut result = [0; 32];
    assert([1, 2, 3, 114, 134, 243].any(|x| x == prefix_length), "Length is not supported");
    if prefix_length == 1 {
        result = std::hash::sha256(prefix(input, [0; 1]));
    }
    if prefix_length == 2 {
        result = std::hash::sha256(prefix(input, [0; 2]));
    }
    if prefix_length == 3 {
        result = std::hash::sha256(prefix(input, [0; 3]));
    }
    if prefix_length == 114 {
        result = std::hash::sha256(prefix(input, [0; 114]));
    }
    if prefix_length == 134 {
        result = std::hash::sha256(prefix(input, [0; 134]));
    }
    if prefix_length == 243 {
        result = std::hash::sha256(prefix(input, [0; 243]));
    }
    result
}


pub fn concatenate_arrays(a: [u8; 37], b: [u8; 32]) -> [u8; 69] {
    let mut result = [0; 69];
    for i in 0..37 {
        result[i] = a[i];
    }
    for i in 0..32 {
        result[i+37] = b[i];
    }
    result
}


fn prefix<N, M>(input: [u8; N], mut result: [u8; M]) -> [u8; M] {
    for i in 0..M {
        result[i] = input[i];
    }
    result
}