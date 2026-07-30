pub struct Public {
    pub value: int
}
struct Secret {
    value: int
}
const HIDDEN: int = 41

pub fn exposed() -> Public {
    return Public { value: 1 }
}
