use std::fmt;

pub enum FlaskError {
    BadRequest(String),
    NotImplemented(String),
}

impl fmt::Debug for FlaskError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("flask::errors::Error")
            .field(&self.get_msg())
            .finish()
    }
}

impl FlaskError {
    pub fn get_msg(&self) -> &str {
        match self {
            FlaskError::BadRequest(s) => s,
            FlaskError::NotImplemented(s) => s,
        }
    }
}
