/// Character set used for generated deal references.
///
/// This is nanoid's URL-safe alphabet (`[A-Za-z0-9_-]`), which is within IG's
/// permitted deal-reference charset. Defining it here — and routing every
/// deal-reference builder through [`get_id`] — keeps the charset in a single
/// place.
const DEAL_REF_ALPHABET: [char; 64] = nanoid::alphabet::SAFE;

/// Length, in characters, of a generated deal reference.
///
/// IG limits deal references to 30 characters.
const DEAL_REF_LENGTH: usize = 30;

/// Generates a unique deal reference as a `String`.
///
/// This function creates a `DEAL_REF_LENGTH`-character unique identifier drawn
/// from `DEAL_REF_ALPHABET` (nanoid's URL-safe alphabet) using the `nanoid`
/// library. The generated identifier is securely random and designed to be
/// collision-resistant, and stays within IG's permitted deal-reference charset.
///
/// # Returns
/// A freshly generated deal reference.
///
/// # Examples
/// ```
/// use ig_client::utils::id::get_id;
///
/// let deal_reference = get_id();
/// assert_eq!(deal_reference.len(), 30);
/// ```
#[must_use]
pub fn get_id() -> String {
    nanoid::nanoid!(DEAL_REF_LENGTH, &DEAL_REF_ALPHABET)
}

/// An IG account id reduced to its first and last character, for logs.
///
/// Account ids identify a client's account (the live-money one included), so
/// the full id never goes to a log line; this masked form is enough to tell two
/// accounts apart when reading one.
///
/// # Examples
/// ```
/// use ig_client::utils::id::mask_account_id;
///
/// assert_eq!(mask_account_id("ABC123"), "A…3");
/// assert_eq!(mask_account_id("A"), "…");
/// assert_eq!(mask_account_id(""), "");
/// ```
#[must_use]
pub fn mask_account_id(account_id: &str) -> String {
    let mut chars = account_id.chars();
    match (chars.next(), chars.next_back()) {
        (Some(first), Some(last)) => format!("{first}…{last}"),
        (Some(_), None) => "…".to_string(),
        _ => String::new(),
    }
}
