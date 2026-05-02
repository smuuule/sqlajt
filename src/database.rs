use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const ID_SIZE: usize = 4;
const USERNAME_SIZE: usize = 32;
const EMAIL_SIZE: usize = 255;
const ROW_SIZE: usize = ID_SIZE + USERNAME_SIZE + EMAIL_SIZE;
const ROWS_PER_PAGE: usize = PAGE_SIZE / ROW_SIZE;
const MAX_PAGES: usize = 100;

pub const PAGE_SIZE: usize = 4096;

pub struct Page {
    pub data: [u8; PAGE_SIZE],
}

impl Page {
    pub fn new() -> Self {
        Page {
            data: [0; PAGE_SIZE],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub id: u32,
    pub username: String,
    pub email: String,
}

impl Row {
    pub fn serialize(&self, dest: &mut [u8]) {
        dest[0..4].copy_from_slice(&self.id.to_ne_bytes());
        dest[4..ROW_SIZE].fill(0);

        let u_bytes = self.username.as_bytes();
        let u_len = u_bytes.len().min(USERNAME_SIZE);
        dest[4..4 + u_len].copy_from_slice(&u_bytes[..u_len]);

        let e_bytes = self.email.as_bytes();
        let e_len = e_bytes.len().min(EMAIL_SIZE);
        dest[4 + USERNAME_SIZE..4 + USERNAME_SIZE + e_len].copy_from_slice(&e_bytes[..e_len]);
    }

    pub fn deserialize(src: &[u8]) -> Self {
        let id = u32::from_ne_bytes(src[0..4].try_into().unwrap());

        let username_slice = &src[4..4 + USERNAME_SIZE];
        let username_len = username_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(USERNAME_SIZE);
        let username = String::from_utf8_lossy(&username_slice[..username_len]).into_owned();

        let email_slice = &src[4 + USERNAME_SIZE..4 + USERNAME_SIZE + EMAIL_SIZE];
        let email_len = email_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(EMAIL_SIZE);
        let email = String::from_utf8_lossy(&email_slice[..email_len]).into_owned();

        Row {
            id,
            username,
            email,
        }
    }
}

pub struct Pager {
    pub file: File,
    pub file_length: u64,
    pub pages: Vec<Option<Page>>,
}

impl Pager {
    pub fn new(mut file: File) -> Self {
        let file_length = file.seek(SeekFrom::End(0)).unwrap_or(0);
        let mut pages = Vec::with_capacity(MAX_PAGES);
        for _ in 0..MAX_PAGES {
            pages.push(None);
        }

        Pager {
            file,
            file_length,
            pages,
        }
    }

    pub fn get_page(&mut self, page_num: usize) -> &mut Page {
        if page_num >= MAX_PAGES {
            panic!("Page number out of bounds.");
        }

        if self.pages[page_num].is_none() {
            let mut page = Page::new();
            let mut num_pages = (self.file_length / PAGE_SIZE as u64) as usize;
            if !self.file_length.is_multiple_of(PAGE_SIZE as u64) {
                num_pages += 1;
            }

            if page_num <= num_pages {
                self.file
                    .seek(SeekFrom::Start((page_num * PAGE_SIZE) as u64))
                    .unwrap();
                let mut bytes_read = 0;
                while bytes_read < PAGE_SIZE {
                    let mut buf = [0u8; PAGE_SIZE];
                    if let Ok(n) = self.file.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        let end = (bytes_read + n).min(PAGE_SIZE);
                        let to_copy = end - bytes_read;
                        page.data[bytes_read..end].copy_from_slice(&buf[..to_copy]);
                        bytes_read += to_copy;
                    } else {
                        break;
                    }
                }
            }
            self.pages[page_num] = Some(page);
        }

        self.pages[page_num].as_mut().unwrap()
    }

    pub fn flush(&mut self, page_num: usize, size: usize) {
        if let Some(page) = &self.pages[page_num] {
            self.file
                .seek(SeekFrom::Start((page_num * PAGE_SIZE) as u64))
                .unwrap();
            self.file.write_all(&page.data[0..size]).unwrap();
            self.file.flush().unwrap();
        }
    }
}

pub struct Table {
    pub pager: Pager,
    pub num_rows: usize,
}

impl Table {
    pub fn new(file: File) -> Self {
        let pager = Pager::new(file);
        let num_rows = (pager.file_length / ROW_SIZE as u64) as usize;
        Table { pager, num_rows }
    }

    fn row_slot(&mut self, row_num: usize) -> (&mut [u8], usize) {
        let page_num = row_num / ROWS_PER_PAGE;
        let page = self.pager.get_page(page_num);
        let row_offset = (row_num % ROWS_PER_PAGE) * ROW_SIZE;
        (&mut page.data, row_offset)
    }

    pub fn close(&mut self) {
        let num_full_pages = self.num_rows / ROWS_PER_PAGE;
        for i in 0..num_full_pages {
            if self.pager.pages[i].is_some() {
                self.pager.flush(i, PAGE_SIZE);
            }
        }

        let num_additional_rows = self.num_rows % ROWS_PER_PAGE;
        if num_additional_rows > 0 {
            let page_num = num_full_pages;
            if self.pager.pages[page_num].is_some() {
                self.pager.flush(page_num, num_additional_rows * ROW_SIZE);
            }
        }
    }
}

pub struct Cursor<'a> {
    pub table: &'a mut Table,
    pub row_num: usize,
    pub end_of_table: bool,
}

impl<'a> Cursor<'a> {
    pub fn table_start(table: &'a mut Table) -> Self {
        let end_of_table = table.num_rows == 0;
        Cursor {
            table,
            row_num: 0,
            end_of_table,
        }
    }

    pub fn table_end(table: &'a mut Table) -> Self {
        let row_num = table.num_rows;
        Cursor {
            table,
            row_num,
            end_of_table: true,
        }
    }

    pub fn get_row(&mut self) -> Row {
        let (page_data, row_offset) = self.table.row_slot(self.row_num);
        Row::deserialize(&page_data[row_offset..row_offset + ROW_SIZE])
    }

    pub fn insert_row(&mut self, row: Row) {
        let (page_data, row_offset) = self.table.row_slot(self.row_num);
        row.serialize(&mut page_data[row_offset..row_offset + ROW_SIZE]);
        self.table.num_rows += 1;
    }

    pub fn advance(&mut self) {
        self.row_num += 1;
        if self.row_num >= self.table.num_rows {
            self.end_of_table = true;
        }
    }
}

pub fn database_open(filename: &str) -> Result<Table, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(Path::new(filename))
        .map_err(|e| format!("Unable to open file: {}", e))?;

    Ok(Table::new(file))
}
