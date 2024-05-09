//! 参考 <https://man7.org/linux/man-pages/man2/ioctl_tty.2.html/>

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub xpixel: u16,
    pub ypixel: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Termios {
    /// Input modes
    pub iflag: u32,
    /// Ouput modes
    pub oflag: u32,
    /// Control modes
    pub cflag: u32,
    /// Local modes
    pub lflag: u32,
    pub line: u8,
    /// Terminal special characters.
    pub cc: [u8; 19],
    // pub cc: [u8; 32],
    // pub ispeed: u32,
    // pub ospeed: u32,
}

// 以下四个和 `struct termios` 相关

/// 获取当前串口设置
pub const TCGETS: usize = 0x5401;
/// 立刻修改当前串口设置
pub const TCSETS: usize = 0x5402;
/// 等待输出缓冲区耗尽再修改串口设置
pub const TCSETSW: usize = 0x5403;
/// 等待输出缓冲区耗尽，丢弃待处理的输入，再修改串口设置
pub const TCSETSF: usize = 0x5404;

// 以下四个和 `struct termio` 相关
/// 获取当前串口设置
pub const TCGETA: usize = 0x5405;
/// 立刻修改当前串口设置
pub const TCSETA: usize = 0x5406;
/// 等待输出缓冲区耗尽再修改串口设置
pub const TCSETAW: usize = 0x5407;
/// 等待输出缓冲区耗尽，丢弃待处理的输入，再修改串口设置
pub const TCSETAF: usize = 0x5408;

/// - 如果终端使用异步串行数据传输，并且 `arg` 为零，则发送 0.25 到 0.5 秒之间的中断（零位流）
/// - 如果终端未使用异步串行数据传输，则发送中断，或者函数返回而不执行任何操作
pub const TCSBRK: usize = 0x5409;

pub const TCXONC: usize = 0x540A;
pub const TCFLSH: usize = 0x540B;
pub const TIOCEXCL: usize = 0x540C;
pub const TIOCNXCL: usize = 0x540D;
pub const TIOCSCTTY: usize = 0x540E;
pub const TIOCGPGRP: usize = 0x540F;
pub const TIOCSPGRP: usize = 0x5410;
pub const TIOCOUTQ: usize = 0x5411;
pub const TIOCSTI: usize = 0x5412;
pub const TIOCGWINSZ: usize = 0x5413;
pub const TIOCSWINSZ: usize = 0x5414;
pub const TIOCMGET: usize = 0x5415;
pub const TIOCMBIS: usize = 0x5416;
pub const TIOCMBIC: usize = 0x5417;
pub const TIOCMSET: usize = 0x5418;
pub const TIOCGSOFTCAR: usize = 0x5419;
pub const TIOCSSOFTCAR: usize = 0x541A;
pub const FIONREAD: usize = 0x541B;
pub const TIOCINQ: usize = FIONREAD;
pub const TIOCLINUX: usize = 0x541C;
pub const TIOCCONS: usize = 0x541D;
pub const TIOCGSERIAL: usize = 0x541E;
pub const TIOCSSERIAL: usize = 0x541F;
pub const TIOCPKT: usize = 0x5420;
pub const FIONBIO: usize = 0x5421;
pub const TIOCNOTTY: usize = 0x5422;
pub const TIOCSETD: usize = 0x5423;
pub const TIOCGETD: usize = 0x5424;
pub const TCSBRKP: usize = 0x5425;
pub const TIOCSBRK: usize = 0x5427;
pub const TIOCCBRK: usize = 0x5428;
pub const TIOCGSID: usize = 0x5429;
pub const TIOCGRS485: usize = 0x542E;
pub const TIOCSRS485: usize = 0x542F;
pub const TIOCGPTN: usize = 0x80045430;
pub const TIOCSPTLCK: usize = 0x40045431;
pub const TIOCGDEV: usize = 0x80045432;
pub const TCGETX: usize = 0x5432;
pub const TCSETX: usize = 0x5433;
pub const TCSETXF: usize = 0x5434;
pub const TCSETXW: usize = 0x5435;
pub const TIOCSIG: usize = 0x40045436;
pub const TIOCVHANGUP: usize = 0x5437;
pub const TIOCGPKT: usize = 0x80045438;
pub const TIOCGPTLCK: usize = 0x80045439;
pub const TIOCGEXCL: usize = 0x80045440;
pub const TIOCGPTPEER: usize = 0x5441;
pub const TIOCGISO7816: usize = 0x80285442;
pub const TIOCSISO7816: usize = 0xc0285443;

pub const FIONCLEX: usize = 0x5450;
pub const FIOCLEX: usize = 0x5451;
pub const FIOASYNC: usize = 0x5452;
pub const TIOCSERCONFIG: usize = 0x5453;
pub const TIOCSERGWILD: usize = 0x5454;
pub const TIOCSERSWILD: usize = 0x5455;
pub const TIOCGLCKTRMIOS: usize = 0x5456;
pub const TIOCSLCKTRMIOS: usize = 0x5457;
pub const TIOCSERGSTRUCT: usize = 0x5458;
pub const TIOCSERGETLSR: usize = 0x5459;
pub const TIOCSERGETMULTI: usize = 0x545A;
pub const TIOCSERSETMULTI: usize = 0x545B;

pub const TIOCMIWAIT: usize = 0x545C;
pub const TIOCGICOUNT: usize = 0x545D;
pub const FIOQSIZE: usize = 0x5460;

pub const TIOCM_LE: usize = 0x001;
pub const TIOCM_DTR: usize = 0x002;
pub const TIOCM_RTS: usize = 0x004;
pub const TIOCM_ST: usize = 0x008;
pub const TIOCM_SR: usize = 0x010;
pub const TIOCM_CTS: usize = 0x020;
pub const TIOCM_CAR: usize = 0x040;
pub const TIOCM_RNG: usize = 0x080;
pub const TIOCM_DSR: usize = 0x100;
pub const TIOCM_CD: usize = TIOCM_CAR;
pub const TIOCM_RI: usize = TIOCM_RNG;
pub const TIOCM_OUT1: usize = 0x2000;
pub const TIOCM_OUT2: usize = 0x4000;
pub const TIOCM_LOOP: usize = 0x8000;

pub const FIOSETOWN: usize = 0x8901;
pub const SIOCSPGRP: usize = 0x8902;
pub const FIOGETOWN: usize = 0x8903;
pub const SIOCGPGRP: usize = 0x8904;
pub const SIOCATMARK: usize = 0x8905;
pub const SIOCGSTAMP: usize = 0x8906;
pub const SIOCGSTAMPNS: usize = 0x8907;
