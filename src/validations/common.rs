use crate::errors::AppError;
use std::collections::HashMap;

pub struct Validator {
    pub errors: HashMap<String, Vec<String>>,
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    pub fn new() -> Self {
        Self {
            errors: HashMap::new(),
        }
    }

    pub fn add_error(&mut self, field: &str, message: &str) {
        self.errors
            .entry(field.to_string())
            .or_default()
            .push(message.to_string());
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn check_presence(
        &mut self,
        field: &str,
        val: Option<&str>,
        label: &str,
    ) -> Option<String> {
        match val {
            Some(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
            _ => {
                self.add_error(field, &format!("{} can't be blank", label));
                None
            }
        }
    }

    pub fn check_presence_generic<T>(
        &mut self,
        field: &str,
        val: Option<T>,
        label: &str,
    ) -> Option<T> {
        match val {
            Some(v) => Some(v),
            _ => {
                self.add_error(field, &format!("{} can't be blank", label));
                None
            }
        }
    }

    pub fn check_length(&mut self, field: &str, val: &str, max_len: usize, label: &str) {
        if val.chars().count() > max_len {
            self.add_error(
                field,
                &format!("{} is too long (maximum is {} characters)", label, max_len),
            );
        }
    }

    pub fn into_result(self) -> Result<(), AppError> {
        if self.has_errors() {
            Err(AppError::Validation(self.errors))
        } else {
            Ok(())
        }
    }
}

pub fn is_valid_email(email: &str) -> bool {
    if let Some(pos) = email.find('@') {
        let (local, domain) = email.split_at(pos);
        let domain = &domain[1..];
        !local.is_empty()
            && !domain.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    } else {
        false
    }
}
