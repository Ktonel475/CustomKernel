use limine::request::FramebufferRequest;

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

pub const COLOR_BG: u32 = 0x001E1E2E;
pub const COLOR_FG: u32 = 0x00CDD6F4;
pub const COLOR_PANIC: u32 = 0x00F38BA8;

static FONT_BYTES: &[u8] = include_bytes!("../default8x16.psfu");

fn psf2_header() -> (usize, usize, usize, usize) {
    let h = FONT_BYTES;
    let header_size = u32::from_le_bytes([h[8], h[9], h[10], h[11]]) as usize;
    let bytes_per_glyph = u32::from_le_bytes([h[20], h[21], h[22], h[23]]) as usize;
    let height = u32::from_le_bytes([h[24], h[25], h[26], h[27]]) as usize;
    let width = u32::from_le_bytes([h[28], h[29], h[30], h[31]]) as usize;
    (header_size, bytes_per_glyph, height, width)
}

fn get_glyph(c: u8, header_size: usize, bytes_per_glyph: usize) -> &'static [u8] {
    let offset = header_size + c as usize * bytes_per_glyph;
    &FONT_BYTES[offset..offset + bytes_per_glyph]
}

pub struct Terminal {
    addr: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    cols: usize,
    rows: usize,
    col: usize,
    row: usize,
    fg: u32,
    bg: u32,
    glyph_h: usize,
    glyph_w: usize,
    bytes_per_glyph: usize,
    header_size: usize,
}

impl Terminal {
    pub fn new() -> Option<Self> {
        let resp = FRAMEBUFFER_REQUEST.response()?;
        let fbs = resp.framebuffers();
        if fbs.is_empty() {
            return None;
        }
        let fb = &fbs[0];

        let (header_size, bytes_per_glyph, glyph_h, glyph_w) = psf2_header();

        let width = fb.width as usize;
        let height = fb.height as usize;
        let pitch = fb.pitch as usize / 4;
        let addr = fb.address() as *mut u32;

        let mut term = Terminal {
            addr,
            width,
            height,
            pitch,
            cols: width / glyph_w,
            rows: height / glyph_h,
            col: 0,
            row: 0,
            fg: COLOR_FG,
            bg: COLOR_BG,
            glyph_h,
            glyph_w,
            bytes_per_glyph,
            header_size,
        };
        term.clear();
        Some(term)
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        unsafe {
            self.addr.add(y * self.pitch + x).write_volatile(color);
        }
    }

    fn draw_glyph(&mut self, col: usize, row: usize, c: u8) {
        let bits = get_glyph(c, self.header_size, self.bytes_per_glyph);
        let px = col * self.glyph_w;
        let py = row * self.glyph_h;
        let fg = self.fg;
        let bg = self.bg;
        let bytes_per_row = (self.glyph_w + 7) / 8;

        for gy in 0..self.glyph_h {
            for gx in 0..self.glyph_w {
                let bytes_idx = gy * bytes_per_row + gx / 8;
                let bit = 7 - (gx % 8);
                let lit = (bits[bytes_idx] >> bit) & 1 != 0;
                self.put_pixel(px + gx, py + gy, if lit { fg } else { bg });
            }
        }
    }

    fn erase_glyph(&mut self, col: usize, row: usize) {
        let px = col * self.glyph_w;
        let py = row * self.glyph_h;
        let bg = self.bg;
        for gy in 0..self.glyph_h {
            for gx in 0..self.glyph_w {
                self.put_pixel(px + gx, py + gy, bg);
            }
        }
    }

    fn scroll(&mut self) {
        let row_len = self.glyph_h * self.pitch;
        unsafe {
            core::ptr::copy(self.addr.add(row_len), self.addr, (self.rows - 1) * row_len);
            let last = self.addr.add((self.rows - 1) * row_len);
            let bg = self.bg;
            for i in 0..row_len {
                last.add(i).write_volatile(bg);
            }
        }
    }

    pub fn clear(&mut self) {
        let bg = self.bg;
        let total = self.height * self.pitch;
        self.col = 0;
        self.row = 0;
        unsafe {
            for i in 0..total {
                self.addr.add(i).write_volatile(bg);
            }
        }
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.cols - 1;
        } else {
            return;
        }
        self.erase_glyph(self.col, self.row);
    }

    pub fn putc(&mut self, c: char) {
        match c {
            '\n' => {
                self.col = 0;
                self.row += 1;
            }
            '\r' => {
                self.col = 0;
            }
            _ => {
                let byte = if c.is_ascii() && c as u8 >= 32 {
                    c as u8
                } else {
                    b'?'
                };
                self.draw_glyph(self.col, self.row, byte);
                self.col += 1;
            }
        }
        if self.col >= self.cols {
            self.col = 0;
            self.row += 1;
        }
        if self.row >= self.rows {
            self.scroll();
            self.row = self.rows - 1;
        }
    }

    pub fn print(&mut self, s: &str) {
        for c in s.chars() {
            self.putc(c);
        }
    }

    pub fn set_color(&mut self, fg: u32, bg: u32) {
        self.fg = fg;
        self.bg = bg;
    }

    pub fn print_hex64(&mut self, mut v: u64) {
        self.print("0x");
        let mut buf = [b'0'; 16];
        for i in (0..16).rev() {
            let nib = (v & 0xf) as u8;
            buf[i] = if nib < 10 {
                b'0' + nib
            } else {
                b'a' + nib - 10
            };
            v >>= 4;
        }
        for &b in &buf {
            self.putc(b as char);
        }
    }

    pub fn print_usize(&mut self, mut v: usize) {
        let mut buf = [b'0'; 20];
        let mut i = 20;
        if v == 0 {
            self.putc('0');
            return;
        }
        while v > 0 {
            i -= 1;
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
        for &b in &buf[i..] {
            self.putc(b as char);
        }
    }
}
