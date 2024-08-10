use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

use bytemuck::Pod;

pub trait IndexingTrait
{
    type Type: Pod;
    fn add(&mut self, elem: &Self::Type, index: usize);
    fn remove(&mut self, elem: &Self::Type, index: usize);
}

#[derive(Debug)]
pub struct DataBase<IndexingType: IndexingTrait>
{
    pub file: File,
    pub vec: Vec<IndexingType::Type>,
    pub indexing: IndexingType,
}

impl<IndexingType: IndexingTrait + Default> DataBase<IndexingType>
{
    pub fn new_or_open(name: &str) -> Result<DataBase<IndexingType>, Box<dyn Error>> {
        let path = Path::new(name);

        let mut file;
        let mut vec = Vec::new();
        let mut indexing = IndexingType::default();

        if path.exists() {
            file = OpenOptions::new().read(true).write(true).open(name)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            
            for (index, chunk) in buf.chunks_exact(std::mem::size_of::<IndexingType::Type>()).enumerate(){
                let elem = *bytemuck::from_bytes(chunk);
                indexing.add(&elem, index);
                vec.push(elem);
            }
        } else {
            file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(name)?;
        }

        Ok(DataBase {
            file,
            vec,
            indexing,
        })
    }
    pub fn write(&mut self, elem: IndexingType::Type) -> io::Result<()> {
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(bytemuck::bytes_of(&elem))?;
        self.indexing.add(&elem, self.vec.len());
        self.vec.push(elem);
        
        Ok(())
    }
    pub fn change(&mut self, elem: IndexingType::Type, index: usize) -> io::Result<()> {
        self.file.seek(SeekFrom::Start((std::mem::size_of::<IndexingType::Type>() * index) as u64))?;
        self.file.write_all(bytemuck::bytes_of(&elem))?;
        self.indexing.remove(self.vec.get(index).unwrap(), index);
        self.indexing.add(&elem, index);
        self.vec[index] = elem;

        Ok(())
    }
}
