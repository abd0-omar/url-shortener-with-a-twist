use validator::ValidateEmail;

#[derive(Debug)]
pub struct RecipientEmail(String);

impl RecipientEmail {
    pub fn parse(s: String) -> Result<RecipientEmail, String> {
        if s.validate_email() {
            Ok(Self(s))
        } else {
            Err(format!("{} is not a valid recipient email.", s))
        }
    }
}

impl AsRef<str> for RecipientEmail {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::RecipientEmail;
    use claims::assert_err;
    use fake::Fake;
    use fake::faker::internet::en::SafeEmail;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn empty_string_is_rejected() {
        let email = "".to_string();
        assert_err!(RecipientEmail::parse(email));
    }

    #[test]
    fn email_missing_at_symbol_is_rejected() {
        let email = "ursuladomain.com".to_string();
        assert_err!(RecipientEmail::parse(email));
    }

    #[test]
    fn email_missing_subject_is_rejected() {
        let email = "@domain.com".to_string();
        assert_err!(RecipientEmail::parse(email));
    }

    #[derive(Debug, Clone)]
    struct ValidEmailFixture(pub String);

    impl quickcheck::Arbitrary for ValidEmailFixture {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut rng = StdRng::seed_from_u64(u64::arbitrary(g));
            let email = SafeEmail().fake_with_rng(&mut rng);

            Self(email)
        }
    }

    #[quickcheck_macros::quickcheck]
    fn valid_emails_are_parsed_successfully(valid_email: ValidEmailFixture) -> bool {
        RecipientEmail::parse(valid_email.0).is_ok()
    }
}
