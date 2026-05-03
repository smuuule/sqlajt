use std::convert::TryInto;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const PAGE_SIZE: usize = 4096;
const MAX_PAGES: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Int,
    Text,
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i32),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub columns: Vec<Column>,
    pub row_size: usize,
}

impl Schema {
    pub fn new(columns: Vec<Column>) -> Self {
        let mut row_size = 0;
        for col in &columns {
            match col.data_type {
                DataType::Int => row_size += 4,
                DataType::Text => row_size += 256,
            }
        }
        Schema { columns, row_size }
    }
}

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

pub struct Pager {
    pub file: File,
    pub pages: Vec<Option<Page>>,
    pub num_pages: u32,
}

impl Pager {
    pub fn new(mut file: File) -> Self {
        let file_length = file.seek(SeekFrom::End(0)).unwrap_or(0);
        let mut pages = Vec::with_capacity(MAX_PAGES);
        for _ in 0..MAX_PAGES {
            pages.push(None);
        }
        let num_pages = if file_length == 0 {
            0
        } else {
            (file_length as usize / PAGE_SIZE) as u32
        };

        Pager {
            file,
            pages,
            num_pages,
        }
    }

    pub fn get_page(&mut self, page_num: u32) -> &mut Page {
        let page_num_us = page_num as usize;
        if self.pages[page_num_us].is_none() {
            let mut page = Page::new();
            if page_num < self.num_pages {
                self.file
                    .seek(SeekFrom::Start((page_num_us * PAGE_SIZE) as u64))
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
            self.pages[page_num_us] = Some(page);
        }
        self.pages[page_num_us].as_mut().unwrap()
    }

    pub fn flush(&mut self, page_num: u32) {
        if let Some(page) = &self.pages[page_num as usize] {
            self.file
                .seek(SeekFrom::Start((page_num as usize * PAGE_SIZE) as u64))
                .unwrap();
            self.file.write_all(&page.data).unwrap();
            self.file.flush().unwrap();
        }
    }
}

pub struct Table {
    pub pager: Pager,
    pub schema: Option<Schema>,
    pub num_rows: u32,
}

impl Table {
    pub fn new(mut pager: Pager) -> Self {
        let mut num_rows = 0;
        let mut schema = None;

        if pager.num_pages > 0 {
            let page = pager.get_page(0);
            num_rows = u32::from_le_bytes(page.data[0..4].try_into().unwrap());
            let num_columns = u32::from_le_bytes(page.data[4..8].try_into().unwrap());

            if num_columns > 0 {
                let mut columns = Vec::new();
                let mut offset = 8;
                for _ in 0..num_columns {
                    let dt = if page.data[offset] == 0 {
                        DataType::Int
                    } else {
                        DataType::Text
                    };
                    offset += 1;

                    let name_len = page.data[offset] as usize;
                    offset += 1;

                    let name = String::from_utf8(page.data[offset..offset + name_len].to_vec())
                        .unwrap_or_default();
                    offset += name_len;

                    columns.push(Column {
                        name,
                        data_type: dt,
                    });
                }
                schema = Some(Schema::new(columns));
            }
        } else {
            // Page 0 for metadata
            pager.num_pages = 1;
            pager.get_page(0);
        }

        Table {
            pager,
            schema,
            num_rows,
        }
    }

    pub fn close(&mut self) {
        self.save_metadata();
        for i in 0..self.pager.num_pages {
            if self.pager.pages[i as usize].is_some() {
                self.pager.flush(i);
            }
        }
    }

    pub fn save_metadata(&mut self) {
        let page = self.pager.get_page(0);
        page.data[0..4].copy_from_slice(&self.num_rows.to_le_bytes());

        if let Some(schema) = &self.schema {
            let num_cols = schema.columns.len() as u32;
            page.data[4..8].copy_from_slice(&num_cols.to_le_bytes());

            let mut offset = 8;
            for col in &schema.columns {
                page.data[offset] = match col.data_type {
                    DataType::Int => 0,
                    DataType::Text => 1,
                };
                offset += 1;

                let name_bytes = col.name.as_bytes();
                page.data[offset] = name_bytes.len() as u8;
                offset += 1;

                page.data[offset..offset + name_bytes.len()].copy_from_slice(name_bytes);
                offset += name_bytes.len();
            }
        } else {
            page.data[4..8].copy_from_slice(&0u32.to_le_bytes());
        }
    }

    pub fn insert_row(&mut self, values: Vec<Value>) -> Result<(), String> {
        let schema = self.schema.as_ref().ok_or("No table created")?;

        let rows_per_page = PAGE_SIZE / schema.row_size;
        let page_num = 1 + (self.num_rows / rows_per_page as u32);

        if page_num >= self.pager.num_pages {
            self.pager.num_pages = page_num + 1;
        }

        let page = self.pager.get_page(page_num);
        let row_offset = ((self.num_rows as usize) % rows_per_page) * schema.row_size;

        let mut byte_offset = row_offset;

        for (i, val) in values.iter().enumerate() {
            let col = &schema.columns[i];
            match (val, &col.data_type) {
                (Value::Int(n), DataType::Int) => {
                    page.data[byte_offset..byte_offset + 4].copy_from_slice(&n.to_le_bytes());
                    byte_offset += 4;
                }
                (Value::Text(s), DataType::Text) => {
                    let bytes = s.as_bytes();
                    let len = std::cmp::min(bytes.len(), 255);
                    page.data[byte_offset] = len as u8;
                    page.data[byte_offset + 1..byte_offset + 1 + len]
                        .copy_from_slice(&bytes[..len]);
                    byte_offset += 256;
                }
                _ => return Err("Type mismatch".to_string()),
            }
        }

        self.num_rows += 1;
        self.save_metadata();
        Ok(())
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
    let pager = Pager::new(file);
    Ok(Table::new(pager))
}
