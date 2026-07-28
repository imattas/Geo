type Byte = u8
type BytePtr = *Byte
type Bytes = [Byte]

fn main() -> int {
    let value: Byte = 42
    let ptr: BytePtr = null
    let values: Bytes = [1, 2, 3]

    return value as int + values[0] as int
}
