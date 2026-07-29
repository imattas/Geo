import std.platform
import std.string

fn main() -> int {
    let os = platform_os()
    let arch = platform_arch()
    let newline = platform_newline()
    let valid = string_len(os) > 0usize && string_len(arch) > 0usize && string_len(newline) > 0usize
    string_free(os)
    string_free(arch)
    string_free(newline)
    if valid {
        return 0
    }
    return 1
}
