use rand::prelude::*;

include!(concat!(env!("OUT_DIR"), "/adjectives.rs"));
include!(concat!(env!("OUT_DIR"), "/animals.rs"));

#[derive(Debug)]
pub struct Generator<'a, R> {
    rng: R,
    adjectives: &'a [&'a str],
    nouns: &'a [&'a str],
}

impl Default for Generator<'static, ThreadRng> {
    fn default() -> Self {
        Generator::new(ADJECTIVES, ANIMALS, rand::thread_rng())
    }
}

impl<'a, R> Generator<'a, R> {
    pub fn new(adjectives: &'a [&'a str], nouns: &'a [&'a str], rng: R) -> Self {
        assert!(!adjectives.is_empty());
        assert!(!nouns.is_empty());
        Generator {
            rng,
            adjectives,
            nouns,
        }
    }

    pub fn with_rng(rng: R) -> Self {
        Self::new(ADJECTIVES, ANIMALS, rng)
    }
}

impl<'a, R> Iterator for Generator<'a, R>
where
    R: Rng,
{
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let adj = self.adjectives.choose(&mut self.rng).unwrap();
        let noun = self.nouns.choose(&mut self.rng).unwrap();
        Some(format!("{}-{}", adj, noun))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use rand::rngs::StdRng;
    use std::sync::Mutex;

    fn assert_send<T: Send>() {}

    #[test]
    fn generator_implements_send() {
        assert_send::<Generator<StdRng>>();
    }

    #[test]
    fn generate_random_names() {
        let mut gen = Generator::default();
        for _ in 0..100 {
            println!("{}", gen.next().unwrap());
        }
    }

    static GEN: Lazy<Mutex<Generator<StdRng>>> =
        Lazy::new(|| Mutex::new(Generator::with_rng(StdRng::from_entropy())));

    #[test]
    fn global_generator() {
        let mut gen = GEN.lock().unwrap();
        for _ in 0..100 {
            println!("{}", gen.next().unwrap());
        }
    }
}
