use super::{recipient_email::RecipientEmail, recipient_name::RecipientName};

pub struct NewRecipient {
    pub name: RecipientName,
    pub email: RecipientEmail,
}
