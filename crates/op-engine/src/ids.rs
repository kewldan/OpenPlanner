//! Идентификаторы элементов в духе ULID: 48 бит времени и 80 бит случайности
//! в кодировке Crockford Base32 (26 символов). Сортируются по времени создания.

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Генератор идентификаторов.
#[derive(Debug, Clone)]
pub struct IdGen {
    state: u64,
    last_time: u64,
    last_random: u128,
}

impl IdGen {
    /// Генератор с источником случайности от времени и окружения.
    #[must_use]
    pub fn new() -> Self {
        let seed = entropy() ^ now_ms().rotate_left(17);
        Self {
            state: seed | 1,
            last_time: 0,
            last_random: 0,
        }
    }

    fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Новый идентификатор.
    pub fn generate(&mut self) -> String {
        const RANDOM_MASK: u128 = (1u128 << 80) - 1;
        let time = now_ms().max(self.last_time);
        // В пределах одной миллисекунды случайная часть увеличивается на 1 —
        // идентификаторы остаются упорядоченными (монотонный ULID).
        let random = if time == self.last_time && self.last_random < RANDOM_MASK {
            self.last_random + 1
        } else {
            (u128::from(self.next_u64()) << 16 | u128::from(self.next_u64() & 0xFFFF))
                & (RANDOM_MASK >> 1)
        };
        self.last_time = time;
        self.last_random = random;
        let value = (u128::from(time & 0xFFFF_FFFF_FFFF) << 80) | random;
        (0..26)
            .rev()
            .map(|i| {
                let index = usize::try_from((value >> (i * 5)) & 0x1F).unwrap_or(0);
                char::from(ALPHABET[index])
            })
            .collect()
    }
}

impl Default for IdGen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> u64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // время в мс положительно и < 2^53
    let ms = js_sys::Date::now() as u64;
    ms
}

#[cfg(target_arch = "wasm32")]
fn entropy() -> u64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // [0, 1) × 2^53
    let r = (js_sys::Math::random() * 9_007_199_254_740_992.0) as u64;
    r
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(not(target_arch = "wasm32"))]
fn entropy() -> u64 {
    use std::hash::{BuildHasher, RandomState};
    RandomState::new().hash_one(std::time::Instant::now())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ids_are_unique_and_sorted() {
        let mut ids = IdGen::new();
        let list: Vec<String> = (0..1000).map(|_| ids.generate()).collect();
        assert!(list.iter().all(|id| id.len() == 26));
        assert_eq!(list.iter().collect::<HashSet<_>>().len(), list.len());
        let mut sorted = list.clone();
        sorted.sort();
        assert_eq!(sorted, list);
    }
}
