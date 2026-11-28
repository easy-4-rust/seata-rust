//! JDBC 源用例共享的 JVM 反射宿主。
use seata_db_mysql::JdbcDriverLoader;
use seata_sqlparser::{JavaObjectError, JavaString, MetadataError};
use std::{
    error::Error,
    fmt::Write as _,
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::Arc,
};

pub(super) struct Host {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
pub(super) type DriverObservation = (String, bool);

impl Host {
    pub(super) fn start() -> Result<Self, Box<dyn Error>> {
        let mut child =
            Command::new(std::env::var_os("SEATA_JDBC_JAVA").ok_or("missing pinned JVM")?)
                .arg(format!(
                    "-Duser.home={}",
                    std::env::var("SEATA_JDBC_JVM_HOME")?
                ))
                .arg("-Djava.io.tmpdir=D:/Temp")
                .arg("-cp")
                .arg(
                    std::env::var_os("SEATA_JDBC_PRIMITIVE_CLASSPATH")
                        .ok_or("missing primitive host classpath")?,
                )
                .arg("JdbcDriverPrimitiveHost")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;
        let input = child.stdin.take().ok_or("missing JVM stdin")?;
        let output = BufReader::new(child.stdout.take().ok_or("missing JVM stdout")?);
        Ok(Self {
            child,
            input,
            output,
        })
    }

    fn exchange(
        &mut self,
        primitive: &str,
        name: Option<&JavaString>,
    ) -> Result<String, MetadataError> {
        let transport = || MetadataError::InvalidCompatibilityData("JVM driver fixture protocol");
        let mut encoded = String::new();
        if let Some(name) = name {
            for unit in name.as_utf16() {
                write!(&mut encoded, "{unit:04x}").map_err(|_| transport())?;
            }
        } else {
            encoded.push('-');
        }
        writeln!(self.input, "{primitive}\t{encoded}").map_err(|_| transport())?;
        self.input.flush().map_err(|_| transport())?;

        let mut row = String::new();
        if self.output.read_line(&mut row).map_err(|_| transport())? == 0 {
            return Err(transport());
        }
        row.truncate(row.trim_end_matches(['\r', '\n']).len());
        if let Some(fields) = row.strip_prefix("error\t") {
            let [class, parents, encoded]: [&str; 3] = fields
                .split('\t')
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| transport())?;
            let message = if encoded == "-" {
                None
            } else {
                let bytes = encoded.as_bytes().chunks_exact(2);
                if !bytes.remainder().is_empty() {
                    return Err(transport());
                }
                let bytes = bytes
                    .map(|pair| {
                        let value = std::str::from_utf8(pair).map_err(|_| transport())?;
                        u8::from_str_radix(value, 16).map_err(|_| transport())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Some(JavaString::from(
                    String::from_utf8(bytes).map_err(|_| transport())?,
                ))
            };
            return Err(MetadataError::ObjectException(Arc::new(
                JavaObjectError::new(class, message).with_superclasses(parents.split(',')),
            )));
        }
        Ok(row)
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl JdbcDriverLoader for Host {
    type Class = ();
    type Driver = DriverObservation;

    fn load_context_class(
        &mut self,
        name: Option<&JavaString>,
    ) -> Result<Option<()>, MetadataError> {
        match self.exchange("context", name)?.as_str() {
            "class" => Ok(Some(())),
            "none" => Ok(None),
            _ => Err(MetadataError::InvalidCompatibilityData(
                "JVM context class result",
            )),
        }
    }

    fn load_default_class(&mut self, name: Option<&JavaString>) -> Result<(), MetadataError> {
        if self.exchange("default", name)? == "class" {
            Ok(())
        } else {
            Err(MetadataError::InvalidCompatibilityData(
                "JVM default class result",
            ))
        }
    }

    fn instantiate_driver(&mut self, (): ()) -> Result<Self::Driver, MetadataError> {
        let response = self.exchange("instantiate", None)?;
        let mut fields = response.split('\t');
        match (fields.next(), fields.next(), fields.next(), fields.next()) {
            (Some("driver"), Some(class), Some("true"), None) => Ok((class.to_owned(), true)),
            (Some("driver"), Some(class), Some("false"), None) => Ok((class.to_owned(), false)),
            _ => Err(MetadataError::InvalidCompatibilityData("JVM Driver result")),
        }
    }
}
