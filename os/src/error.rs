//! Common error types for TrainOS
//!
//! Provides Linux-compatible error codes

/// Linux error codes (errno)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Success
    Ok = 0,
    /// Operation not permitted
    EPERM = 1,
    /// No such file or directory
    ENOENT = 2,
    /// No such process
    ESRCH = 3,
    /// Interrupted system call
    EINTR = 4,
    /// I/O error
    EIO = 5,
    /// No such device or address
    ENXIO = 6,
    /// Argument list too long
    E2BIG = 7,
    /// Exec format error
    ENOEXEC = 8,
    /// Bad file number
    EBADF = 9,
    /// No child processes
    ECHILD = 10,
    /// Try again
    EAGAIN = 11,
    /// Out of memory
    ENOMEM = 12,
    /// Permission denied
    EACCES = 13,
    /// Bad address
    EFAULT = 14,
    /// Block device required
    ENOTBLK = 15,
    /// Device or resource busy
    EBUSY = 16,
    /// File exists
    EEXIST = 17,
    /// Cross-device link
    EXDEV = 18,
    /// No such device
    ENODEV = 19,
    /// Not a directory
    ENOTDIR = 20,
    /// Is a directory
    EISDIR = 21,
    /// Invalid argument
    EINVAL = 22,
    /// File table overflow
    ENFILE = 23,
    /// Too many open files
    EMFILE = 24,
    /// Not a typewriter
    ENOTTY = 25,
    /// Text file busy
    ETXTBSY = 26,
    /// File too large
    EFBIG = 27,
    /// No space left on device
    ENOSPC = 28,
    /// Illegal seek
    ESPIPE = 29,
    /// Read-only file system
    EROFS = 30,
    /// Too many links
    EMLINK = 31,
    /// Broken pipe
    EPIPE = 32,
    /// Math argument out of domain
    EDOM = 33,
    /// Math result not representable
    ERANGE = 34,
    /// Resource deadlock would occur
    EDEADLK = 35,
    /// File name too long
    ENAMETOOLONG = 36,
    /// No record locks available
    ENOLCK = 37,
    /// Invalid system call number
    ENOSYS = 38,
    /// Directory not empty
    ENOTEMPTY = 39,
    /// Too many symbolic links encountered
    ELOOP = 40,
    /// Operation would block
    EWOULDBLOCK = 41,
    /// No message of desired type
    ENOMSG = 42,
    /// Identifier removed
    EIDRM = 43,
    /// Channel number out of range
    ECHRNG = 44,
    /// Level 2 not synchronized
    EL2NSYNC = 45,
    /// Level 3 halted
    EL3HLT = 46,
    /// Level 3 reset
    EL3RST = 47,
    /// Link number out of range
    ELNRNG = 48,
    /// Protocol driver not attached
    EUNATCH = 49,
    /// No CSI structure available
    ENOCSI = 50,
    /// Level 2 halted
    EL2HLT = 51,
    /// Invalid exchange
    EBADE = 52,
    /// Invalid request descriptor
    EBADR = 53,
    /// Exchange full
    EBQUOT = 54,
    /// Bad anchor node
    ENOANO = 55,
    /// No data (for no data read)
    ENODATA = 56,
    /// Data link timeout
    ETIME = 57,
    /// No data available
    ENONET = 58,
    /// Machine is not on the network
    ENOPKG = 59,
    /// Object is remote
    EREMOTE = 60,
    /// Device not a stream
    ENOSTR = 61,
    /// No process background group
    EBACKGROUND = 62,
    /// Stale remote file handle
    ESTALE = 63,
    /// Needs aggregation layer
    EUCLEAN = 64,
    /// Not a data message
    ENOTUNIQ = 65,
    /// Name to be translated has no alias
    EBADFD = 66,
    /// Remote address changed
    ERECALLCONFLICT = 67,
    /// Nameserver cannot be found
    ENOMEDIUM = 68,
    ///-media format found, e.g. fileid mismatching
    EMEDIUMTYPE = 69,
    /// Value too large for defined data type
    EOVERFLOW = 70,
    /// Name suppressed by privacy settings
    ENOTRECOVERABLE = 71,
    /// State not recoverable
    EOWNERDEAD = 72,
    /// Unknown error
    UNKNOWN = 999,
}

impl Error {
    /// Convert errno to Error
    pub fn from_errno(errno: isize) -> Self {
        match errno {
            0 => Error::Ok,
            1 => Error::EPERM,
            2 => Error::ENOENT,
            3 => Error::ESRCH,
            4 => Error::EINTR,
            5 => Error::EIO,
            6 => Error::ENXIO,
            7 => Error::E2BIG,
            8 => Error::ENOEXEC,
            9 => Error::EBADF,
            10 => Error::ECHILD,
            11 => Error::EAGAIN,
            12 => Error::ENOMEM,
            13 => Error::EACCES,
            14 => Error::EFAULT,
            15 => Error::ENOTBLK,
            16 => Error::EBUSY,
            17 => Error::EEXIST,
            18 => Error::EXDEV,
            19 => Error::ENODEV,
            20 => Error::ENOTDIR,
            21 => Error::EISDIR,
            22 => Error::EINVAL,
            23 => Error::ENFILE,
            24 => Error::EMFILE,
            25 => Error::ENOTTY,
            26 => Error::ETXTBSY,
            27 => Error::EFBIG,
            28 => Error::ENOSPC,
            29 => Error::ESPIPE,
            30 => Error::EROFS,
            31 => Error::EMLINK,
            32 => Error::EPIPE,
            33 => Error::EDOM,
            34 => Error::ERANGE,
            35 => Error::EDEADLK,
            36 => Error::ENAMETOOLONG,
            37 => Error::ENOLCK,
            38 => Error::ENOSYS,
            39 => Error::ENOTEMPTY,
            40 => Error::ELOOP,
            41 => Error::EWOULDBLOCK,
            42 => Error::ENOMSG,
            43 => Error::EIDRM,
            44 => Error::ECHRNG,
            45 => Error::EL2NSYNC,
            46 => Error::EL3HLT,
            47 => Error::EL3RST,
            48 => Error::ELNRNG,
            49 => Error::EUNATCH,
            50 => Error::ENOCSI,
            51 => Error::EL2HLT,
            52 => Error::EBADE,
            53 => Error::EBADR,
            54 => Error::EBQUOT,
            55 => Error::ENOANO,
            56 => Error::ENODATA,
            57 => Error::ETIME,
            58 => Error::ENONET,
            59 => Error::ENOPKG,
            60 => Error::EREMOTE,
            61 => Error::ENOSTR,
            62 => Error::EBACKGROUND,
            63 => Error::ESTALE,
            64 => Error::EUCLEAN,
            65 => Error::ENOTUNIQ,
            66 => Error::EBADFD,
            67 => Error::ERECALLCONFLICT,
            68 => Error::ENOMEDIUM,
            69 => Error::EMEDIUMTYPE,
            70 => Error::EOVERFLOW,
            71 => Error::ENOTRECOVERABLE,
            72 => Error::EOWNERDEAD,
            _ => Error::UNKNOWN,
        }
    }

    /// Convert Error to negative errno
    pub fn to_errno(&self) -> isize {
        -(*self as isize)
    }

    /// Get error message
    pub fn message(&self) -> &str {
        match self {
            Error::Ok => "Success",
            Error::EPERM => "Operation not permitted",
            Error::ENOENT => "No such file or directory",
            Error::ESRCH => "No such process",
            Error::EINTR => "Interrupted system call",
            Error::EIO => "I/O error",
            Error::ENXIO => "No such device or address",
            Error::E2BIG => "Argument list too long",
            Error::ENOEXEC => "Exec format error",
            Error::EBADF => "Bad file number",
            Error::ECHILD => "No child processes",
            Error::EAGAIN => "Try again",
            Error::ENOMEM => "Out of memory",
            Error::EACCES => "Permission denied",
            Error::EFAULT => "Bad address",
            Error::ENOTBLK => "Block device required",
            Error::EBUSY => "Device or resource busy",
            Error::EEXIST => "File exists",
            Error::EXDEV => "Cross-device link",
            Error::ENODEV => "No such device",
            Error::ENOTDIR => "Not a directory",
            Error::EISDIR => "Is a directory",
            Error::EINVAL => "Invalid argument",
            Error::ENFILE => "File table overflow",
            Error::EMFILE => "Too many open files",
            Error::ENOTTY => "Not a typewriter",
            Error::ETXTBSY => "Text file busy",
            Error::EFBIG => "File too large",
            Error::ENOSPC => "No space left on device",
            Error::ESPIPE => "Illegal seek",
            Error::EROFS => "Read-only file system",
            Error::EMLINK => "Too many links",
            Error::EPIPE => "Broken pipe",
            Error::EDOM => "Math argument out of domain",
            Error::ERANGE => "Math result not representable",
            Error::EDEADLK => "Resource deadlock would occur",
            Error::ENAMETOOLONG => "File name too long",
            Error::ENOLCK => "No record locks available",
            Error::ENOSYS => "Invalid system call number",
            Error::ENOTEMPTY => "Directory not empty",
            Error::ELOOP => "Too many symbolic links encountered",
            Error::EWOULDBLOCK => "Operation would block",
            Error::ENOMSG => "No message of desired type",
            Error::EIDRM => "Identifier removed",
            Error::ECHRNG => "Channel number out of range",
            Error::EL2NSYNC => "Level 2 not synchronized",
            Error::EL3HLT => "Level 3 halted",
            Error::EL3RST => "Level 3 reset",
            Error::ELNRNG => "Link number out of range",
            Error::EUNATCH => "Protocol driver not attached",
            Error::ENOCSI => "No CSI structure available",
            Error::EL2HLT => "Level 2 halted",
            Error::EBADE => "Invalid exchange",
            Error::EBADR => "Invalid request descriptor",
            Error::EBQUOT => "Exchange full",
            Error::ENOANO => "Bad anchor node",
            Error::ENODATA => "No data (for no data read)",
            Error::ETIME => "Data link timeout",
            Error::ENONET => "No data available",
            Error::ENOPKG => "Nameserver cannot be found",
            Error::EREMOTE => "Object is remote",
            Error::ENOSTR => "Device not a stream",
            Error::EBACKGROUND => "No process background group",
            Error::ESTALE => "Stale remote file handle",
            Error::EUCLEAN => "Needs aggregation layer",
            Error::ENOTUNIQ => "Not a data message",
            Error::EBADFD => "Name to be translated has no alias",
            Error::ERECALLCONFLICT => "Remote address changed",
            Error::ENOMEDIUM => "Name to be translated has no alias",
            Error::EMEDIUMTYPE => "Media format found",
            Error::EOVERFLOW => "Value too large for defined data type",
            Error::ENOTRECOVERABLE => "Name suppressed by privacy settings",
            Error::EOWNERDEAD => "State not recoverable",
            Error::UNKNOWN => "Unknown error",
        }
    }
}

impl From<Error> for isize {
    fn from(err: Error) -> Self {
        err.to_errno()
    }
}