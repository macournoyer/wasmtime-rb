use super::root;
use crate::error;
use magnus::{
    function, gc, method, typed_data::Obj, DataTypeFunctions, Error, Module, Object, RArray, RHash,
    RString, TypedData,
};
use std::cell::RefCell;
use std::{fs::File, path::PathBuf};
use wasi_common::pipe::ReadPipe;

enum ReadStream {
    Inherit,
    Path(RString),
    String(RString),
}

impl ReadStream {
    pub fn mark(&self) {
        match self {
            Self::Inherit => (),
            Self::Path(s) => gc::mark(*s),
            Self::String(s) => gc::mark(*s),
        }
    }
}

enum WriteStream {
    Inherit,
    Path(RString),
}
impl WriteStream {
    pub fn mark(&self) {
        match self {
            Self::Inherit => (),
            Self::Path(v) => gc::mark(*v),
        }
    }
}

#[derive(Default)]
struct WasiCtxBuilderInner {
    stdin: Option<ReadStream>,
    stdout: Option<WriteStream>,
    stderr: Option<WriteStream>,
    env: Option<RHash>,
    args: Option<RArray>,
}

impl WasiCtxBuilderInner {
    pub fn mark(&self) {
        if let Some(v) = self.stdin.as_ref() {
            v.mark();
        }
        if let Some(v) = self.stdout.as_ref() {
            v.mark();
        }
        if let Some(v) = self.stderr.as_ref() {
            v.mark();
        }
        if let Some(v) = self.env.as_ref() {
            gc::mark(*v);
        }
        if let Some(v) = self.args.as_ref() {
            gc::mark(*v);
        }
    }
}

/// @yard
/// WASI context builder to be sent as {Store#new}’s +wasi_ctx+ keyword argument.
///
/// Instance methods mutate the current object and return +self+.
///
/// @see https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/sync/struct.WasiCtxBuilder.html
///   Wasmtime's Rust doc
// #[derive(Debug)]
#[derive(Default, TypedData)]
#[magnus(class = "Wasmtime::WasiCtxBuilder", size, mark, free_immediatly)]
pub struct WasiCtxBuilder {
    inner: RefCell<WasiCtxBuilderInner>,
}

impl DataTypeFunctions for WasiCtxBuilder {
    fn mark(&self) {
        self.inner.borrow().mark();
    }
}

type RbSelf = Obj<WasiCtxBuilder>;

impl WasiCtxBuilder {
    /// @yard
    /// Create a new {WasiCtxBuilder}. By default, it has nothing: no stdin/out/err,
    /// no env, no argv, no file access.
    /// @return [WasiCtxBuilder]
    pub fn new() -> Self {
        Self::default()
    }

    /// @yard
    /// Inherit stdin from the current Ruby process.
    /// @return [WasiCtxBuilder] +self+
    pub fn inherit_stdin(rb_self: RbSelf) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.stdin = Some(ReadStream::Inherit);
        rb_self
    }

    /// @yard
    /// Set stdin to read from the specified file.
    /// @param path [String] The path of the file to read from.
    /// @def set_stdin_file(path)
    /// @return [WasiCtxBuilder] +self+
    pub fn set_stdin_file(rb_self: RbSelf, path: RString) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.stdin = Some(ReadStream::Path(path));
        rb_self
    }

    /// @yard
    /// Set stdin to the specified String.
    /// @param content [String]
    /// @def set_stdin_string(content)
    /// @return [WasiCtxBuilder] +self+
    pub fn set_stdin_string(rb_self: RbSelf, content: RString) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.stdin = Some(ReadStream::String(content));
        rb_self
    }

    /// @yard
    /// Inherit stdout from the current Ruby process.
    /// @return [WasiCtxBuilder] +self+
    pub fn inherit_stdout(rb_self: RbSelf) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.stdout = Some(WriteStream::Inherit);
        rb_self
    }

    /// @yard
    /// Set stdout to write to a file. Will truncate the file if it exists,
    /// otherwise try to create it.
    /// @param path [String] The path of the file to write to.
    /// @def set_stdout_file(path)
    /// @return [WasiCtxBuilder] +self+
    pub fn set_stdout_file(rb_self: RbSelf, path: RString) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.stdout = Some(WriteStream::Path(path));
        rb_self
    }

    /// @yard
    /// Inherit stderr from the current Ruby process.
    /// @return [WasiCtxBuilder] +self+
    pub fn inherit_stderr(rb_self: RbSelf) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.stderr = Some(WriteStream::Inherit);
        rb_self
    }

    /// @yard
    /// Set stderr to write to a file. Will truncate the file if it exists,
    /// otherwise try to create it.
    /// @param path [String] The path of the file to write to.
    /// @def set_stderr_file(path)
    /// @return [WasiCtxBuilder] +self+
    pub fn set_stderr_file(rb_self: RbSelf, path: RString) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.stderr = Some(WriteStream::Path(path));
        rb_self
    }

    /// @yard
    /// Set env to the specified +Hash+.
    /// @param env [Hash<String, String>]
    /// @def set_env(env)
    /// @return [WasiCtxBuilder] +self+
    pub fn set_env(rb_self: RbSelf, env: RHash) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.env = Some(env);
        rb_self
    }

    /// @yard
    /// Set the arguments (argv) to the specified +Array+.
    /// @param args [Array<String>]
    /// @def set_argv(args)
    /// @return [WasiCtxBuilder] +self+
    pub fn set_argv(rb_self: RbSelf, argv: RArray) -> RbSelf {
        let mut inner = rb_self.get().inner.borrow_mut();
        inner.args = Some(argv);
        rb_self
    }

    pub fn build_context(&self) -> Result<wasmtime_wasi::WasiCtx, Error> {
        let mut builder = wasmtime_wasi::WasiCtxBuilder::new();
        let inner = self.inner.borrow();

        if let Some(stdin) = inner.stdin.as_ref() {
            builder = match stdin {
                ReadStream::Inherit => builder.inherit_stdin(),
                ReadStream::Path(path) => builder.stdin(file_r(*path).map(wasi_file)?),
                ReadStream::String(input) => {
                    // SAFETY: &[u8] copied before calling in to Ruby, no GC can happen before.
                    let pipe = ReadPipe::from(unsafe { input.as_slice() });
                    builder.stdin(Box::new(pipe))
                }
            }
        }

        if let Some(stdout) = inner.stdout.as_ref() {
            builder = match stdout {
                WriteStream::Inherit => builder.inherit_stdout(),
                WriteStream::Path(path) => builder.stdout(file_w(*path).map(wasi_file)?),
            }
        }

        if let Some(stderr) = inner.stderr.as_ref() {
            builder = match stderr {
                WriteStream::Inherit => builder.inherit_stderr(),
                WriteStream::Path(path) => builder.stderr(file_w(*path).map(wasi_file)?),
            }
        }

        if let Some(args) = inner.args.as_ref() {
            // SAFETY: no gc can happen nor do we write to `args`.
            for item in unsafe { args.as_slice() } {
                let arg = item.try_convert::<RString>()?;
                // SAFETY: &str copied before calling in to Ruby, no GC can happen before.
                let arg = unsafe { arg.as_str() }?;
                builder = builder.arg(arg).map_err(|e| error!("{}", e))?
            }
        }

        if let Some(env_hash) = inner.env.as_ref() {
            let env_vec: Vec<(String, String)> = env_hash.to_vec()?;
            builder = builder.envs(&env_vec).map_err(|e| error!("{}", e))?;
        }

        Ok(builder.build())
    }
}

fn file_r(path: RString) -> Result<File, Error> {
    // SAFETY: &str copied before calling in to Ruby, no GC can happen before.
    File::open(PathBuf::from(unsafe { path.as_str()? }))
        .map_err(|e| error!("Failed to open file {}\n{}", path, e))
}

fn file_w(path: RString) -> Result<File, Error> {
    // SAFETY: &str copied before calling in to Ruby, no GC can happen before.
    File::create(unsafe { path.as_str()? })
        .map_err(|e| error!("Failed to write to file {}\n{}", path, e))
}

fn wasi_file(file: File) -> Box<wasi_cap_std_sync::file::File> {
    let file = cap_std::fs::File::from_std(file);
    let file = wasi_cap_std_sync::file::File::from_cap_std(file);
    Box::new(file)
}

pub fn init() -> Result<(), Error> {
    let class = root().define_class("WasiCtxBuilder", Default::default())?;
    class.define_singleton_method("new", function!(WasiCtxBuilder::new, 0))?;

    class.define_method("inherit_stdin", method!(WasiCtxBuilder::inherit_stdin, 0))?;
    class.define_method("set_stdin_file", method!(WasiCtxBuilder::set_stdin_file, 1))?;
    class.define_method(
        "set_stdin_string",
        method!(WasiCtxBuilder::set_stdin_string, 1),
    )?;

    class.define_method("inherit_stdout", method!(WasiCtxBuilder::inherit_stdout, 0))?;
    class.define_method(
        "set_stdout_file",
        method!(WasiCtxBuilder::set_stdout_file, 1),
    )?;

    class.define_method("inherit_stderr", method!(WasiCtxBuilder::inherit_stderr, 0))?;
    class.define_method(
        "set_stderr_file",
        method!(WasiCtxBuilder::set_stderr_file, 1),
    )?;

    class.define_method("set_env", method!(WasiCtxBuilder::set_env, 1))?;

    class.define_method("set_argv", method!(WasiCtxBuilder::set_argv, 1))?;

    Ok(())
}
