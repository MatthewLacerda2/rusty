//! A brief, its canonical form, and the digest that names what it makes (#384).
//!
//! The digest answers one question: *have I already asked for this?* Only the input
//! can answer it, so the brief is hashed, never the output. And the hash is only
//! honest if the same brief always serialises to the same text, which is what
//! [`canonical`] guarantees whatever order a struct declares its fields in or a file
//! lists its keys in.

use serde::Serialize;
use serde_json::{Map, Value};

/// What one generation asks a provider for: the prompt, the voice, the model, the
/// duration, the seed. Its serialised fields *are* the request, so every field that
/// changes the output belongs in it, and nothing about *when* does (a timestamp
/// would make an unchanged brief look edited and pay for it again).
///
/// Mark optional fields `#[serde(skip_serializing_if = "Option::is_none")]` the way
/// the song and patch documents do; [`canonical`] drops a `null` either way.
pub trait Brief: Serialize {
    /// What kind of asset this makes (`speech`, `sfx`, `music`). It leads the file
    /// name and is part of the digest, so two kinds never share an address.
    fn kind(&self) -> &'static str;

    /// The file extension the provider's output is saved under, without the dot.
    fn extension(&self) -> &'static str;
}

/// The brief as a canonical JSON value: object keys sorted, `null`s dropped.
///
/// Dropping `null` is what lets a brief grow an optional field without moving the
/// address of every brief written before it existed.
pub fn canonical<B: Brief + ?Sized>(brief: &B) -> serde_json::Result<Value> {
    serde_json::to_value(brief).map(normalise)
}

/// The digest naming `brief`'s output: sha256, lowercase hex.
pub fn digest<B: Brief + ?Sized>(brief: &B) -> serde_json::Result<String> {
    Ok(digest_of(brief.kind(), &canonical(brief)?))
}

/// The digest of a brief already in canonical form, as a sidecar stores it.
///
/// Hashed as labelled lines, the shape scorsese's bake and brief fingerprints use,
/// so the hashed text is something a person can print and read, and a kind can
/// never run into the brief after it.
pub fn digest_of(kind: &str, canonical: &Value) -> String {
    scorsese_core::hash_bytes(fingerprint(kind, canonical).as_bytes())
}

/// The text [`digest_of`] hashes.
pub fn fingerprint(kind: &str, canonical: &Value) -> String {
    format!("rusty-brief\nkind:{kind}\nbrief:{canonical}\n")
}

/// Sort every object's keys and drop its `null` members, all the way down. Sorted
/// explicitly rather than trusting `serde_json::Map`'s order, which a crate
/// elsewhere in the tree could flip to insertion order (`preserve_order`).
fn normalise(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map
                .into_iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k, normalise(v)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().collect::<Map<_, _>>())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalise).collect()),
        other => other,
    }
}
