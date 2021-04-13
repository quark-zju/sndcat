use std::borrow::Cow;
use std::fmt;

/// A node in the parsed AST.
#[derive(Clone)]
pub enum Expr {
    /// A plain string name.
    Name(String),

    /// A function call.
    Fn(Cow<'static, str>, Vec<Expr>),
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expr::Name(s) => f.write_str(&s)?,
            Expr::Fn(name, args) => {
                if args.is_empty() {
                    f.write_str(name)?;
                    f.write_str("()")?;
                } else {
                    let mut list = f.debug_tuple(&name);
                    for arg in args {
                        list.field(arg);
                    }
                    list.finish()?;
                }
            }
        }
        Ok(())
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

impl Expr {
    /// Parse AST from a string.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        crate::parser::parse(s).map_err(|e| anyhow::format_err!("{}", e))
    }

    /// Parse as an integer.
    pub fn to_i64(&self) -> anyhow::Result<i64> {
        let result = match self {
            Expr::Name(n) => match n.as_str() {
                "48k" => 48000,
                "44k" => 44100,
                "32k" => 32000,
                "24k" => 24000,
                "16k" => 16000,
                "8k" => 8000,
                "mono" => 1,
                "stereo" => 2,
                n => n.parse()?,
            },
            _ => anyhow::bail!("{} is not an integer", self),
        };
        Ok(result)
    }

    /// Parse as a plain string.
    pub fn to_str(&self) -> anyhow::Result<&str> {
        let result = match self {
            Expr::Name(n) => n,
            _ => anyhow::bail!("{} is not a string", self),
        };
        Ok(result)
    }
}
