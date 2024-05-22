//! 参考
//! - <https://man7.org/linux/man-pages/man2/ioctl_tty.2.html/>
//! - musl arch/generic/bits/termios.h

// NOTE: 关于 termios 的定义，musl 和 linux 似乎略有不同
// musl 中 NCCS 为 32，且 termios 结构体含有 __c_ispeed 和 __c_ospeed 两个字段
// linux 中 NCCS 为 19 且不含这两个字段

pub const NCCS: usize = 19;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Termios {
    /// 输入模式
    pub iflag: u32,
    /// 输出模式
    pub oflag: u32,
    /// 控制模式
    pub cflag: u32,
    /// 本地模式？
    pub lflag: u32,
    pub line: u8,
    /// 终端的特殊字符
    pub cc: [u8; NCCS],
    // pub _ispeed: u32,
    // pub _ospeed: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub xpixel: u16,
    pub ypixel: u16,
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
/// 获取该终端上前台进程组的 pgid
pub const TIOCGPGRP: usize = 0x540F;
pub const TIOCSPGRP: usize = 0x5410;
pub const TIOCOUTQ: usize = 0x5411;
pub const TIOCSTI: usize = 0x5412;
/// 获取窗口大小
pub const TIOCGWINSZ: usize = 0x5413;
/// 设置窗口大小
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

pub const VINTR: u32 = 0;
pub const VQUIT: u32 = 1;
pub const VERASE: u32 = 2;
pub const VKILL: u32 = 3;
pub const VEOF: u32 = 4;
pub const VTIME: u32 = 5;
pub const VMIN: u32 = 6;
pub const VSWTC: u32 = 7;
pub const VSTART: u32 = 8;
pub const VSTOP: u32 = 9;
pub const VSUSP: u32 = 10;
pub const VEOL: u32 = 11;
pub const VREPRINT: u32 = 12;
pub const VDISCARD: u32 = 13;
pub const VWERASE: u32 = 14;
pub const VLNEXT: u32 = 15;
pub const VEOL2: u32 = 16;

pub const IGNBRK: u32 = 0o000001;
pub const BRKINT: u32 = 0o000002;
pub const IGNPAR: u32 = 0o000004;
pub const PARMRK: u32 = 0o000010;
pub const INPCK: u32 = 0o000020;
pub const ISTRIP: u32 = 0o000040;
pub const INLCR: u32 = 0o000100;
pub const IGNCR: u32 = 0o000200;
pub const ICRNL: u32 = 0o000400;
pub const IUCLC: u32 = 0o001000;
pub const IXON: u32 = 0o002000;
pub const IXANY: u32 = 0o004000;
pub const IXOFF: u32 = 0o010000;
pub const IMAXBEL: u32 = 0o020000;
pub const IUTF8: u32 = 0o040000;

pub const OPOST: u32 = 0o000001;
pub const OLCUC: u32 = 0o000002;
pub const ONLCR: u32 = 0o000004;
pub const OCRNL: u32 = 0o000010;
pub const ONOCR: u32 = 0o000020;
pub const ONLRET: u32 = 0o000040;
pub const OFILL: u32 = 0o000100;
pub const OFDEL: u32 = 0o000200;
pub const NLDLY: u32 = 0o000400;
pub const NL0: u32 = 0o000000;
pub const NL1: u32 = 0o000400;
pub const CRDLY: u32 = 0o003000;
pub const CR0: u32 = 0o000000;
pub const CR1: u32 = 0o001000;
pub const CR2: u32 = 0o002000;
pub const CR3: u32 = 0o003000;
pub const TABDLY: u32 = 0o014000;
pub const TAB0: u32 = 0o000000;
pub const TAB1: u32 = 0o004000;
pub const TAB2: u32 = 0o010000;
pub const TAB3: u32 = 0o014000;
pub const BSDLY: u32 = 0o020000;
pub const BS0: u32 = 0o000000;
pub const BS1: u32 = 0o020000;
pub const FFDLY: u32 = 0o100000;
pub const FF0: u32 = 0o000000;
pub const FF1: u32 = 0o100000;

pub const VTDLY: u32 = 0o040000;
pub const VT0: u32 = 0o000000;
pub const VT1: u32 = 0o040000;

pub const B0: u32 = 0o000000;
pub const B50: u32 = 0o000001;
pub const B75: u32 = 0o000002;
pub const B110: u32 = 0o000003;
pub const B134: u32 = 0o000004;
pub const B150: u32 = 0o000005;
pub const B200: u32 = 0o000006;
pub const B300: u32 = 0o000007;
pub const B600: u32 = 0o000010;
pub const B1200: u32 = 0o000011;
pub const B1800: u32 = 0o000012;
pub const B2400: u32 = 0o000013;
pub const B4800: u32 = 0o000014;
pub const B9600: u32 = 0o000015;
pub const B19200: u32 = 0o000016;
pub const B38400: u32 = 0o000017;

pub const B57600: u32 = 0o010001;
pub const B115200: u32 = 0o010002;
pub const B230400: u32 = 0o010003;
pub const B460800: u32 = 0o010004;
pub const B500000: u32 = 0o010005;
pub const B576000: u32 = 0o010006;
pub const B921600: u32 = 0o010007;
pub const B1000000: u32 = 0o010010;
pub const B1152000: u32 = 0o010011;
pub const B1500000: u32 = 0o010012;
pub const B2000000: u32 = 0o010013;
pub const B2500000: u32 = 0o010014;
pub const B3000000: u32 = 0o010015;
pub const B3500000: u32 = 0o010016;
pub const B4000000: u32 = 0o010017;

pub const CSIZE: u32 = 0o000060;
pub const CS5: u32 = 0o000000;
pub const CS6: u32 = 0o000020;
pub const CS7: u32 = 0o000040;
pub const CS8: u32 = 0o000060;
pub const CSTOPB: u32 = 0o000100;
pub const CREAD: u32 = 0o000200;
pub const PARENB: u32 = 0o000400;
pub const PARODD: u32 = 0o001000;
pub const HUPCL: u32 = 0o002000;
pub const CLOCAL: u32 = 0o004000;

pub const ISIG: u32 = 0o000001;
pub const ICANON: u32 = 0o000002;
pub const ECHO: u32 = 0o000010;
pub const ECHOE: u32 = 0o000020;
pub const ECHOK: u32 = 0o000040;
pub const ECHONL: u32 = 0o000100;
pub const NOFLSH: u32 = 0o000200;
pub const TOSTOP: u32 = 0o000400;
pub const IEXTEN: u32 = 0o100000;

pub const TCOOFF: u32 = 0;
pub const TCOON: u32 = 1;
pub const TCIOFF: u32 = 2;
pub const TCION: u32 = 3;

pub const TCIFLUSH: u32 = 0;
pub const TCOFLUSH: u32 = 1;
pub const TCIOFLUSH: u32 = 2;

pub const TCSANOW: u32 = 0;
pub const TCSADRAIN: u32 = 1;
pub const TCSAFLUSH: u32 = 2;

pub const EXTA: u32 = 0o000016;
pub const EXTB: u32 = 0o000017;
pub const CBAUD: u32 = 0o010017;
pub const CBAUDEX: u32 = 0o010000;
pub const CIBAUD: u32 = 0o02003600000;
pub const CMSPAR: u32 = 0o10000000000;
pub const CRTSCTS: u32 = 0o20000000000;

pub const XCASE: u32 = 0o000004;
pub const ECHOCTL: u32 = 0o001000;
pub const ECHOPRT: u32 = 0o002000;
pub const ECHOKE: u32 = 0o004000;
pub const FLUSHO: u32 = 0o010000;
pub const PENDIN: u32 = 0o040000;
pub const EXTPROC: u32 = 0o200000;

pub const XTABS: u32 = 0o014000;
