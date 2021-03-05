pub fn roundup_div(lhs: u64, rhs: u64) -> u64 {
    let ret = lhs / rhs;
    if lhs % rhs == 0 {
        ret
    } else {
        ret + 1
    }
}
