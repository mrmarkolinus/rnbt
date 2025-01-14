// ## Author
// - mrmarkolinus
//
// ## Date
// - 2023-12-17
//
// ## File Version
// - 1.0.0
//
// ## Changelog
// - 1.0.0: Initial version

pub mod nbt_tag;
pub mod file_parser;
pub mod region;
pub mod generic_bin;
pub mod blocks;
pub mod chunk_format;

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use pyo3::types::{PyDict, PyList};
use log::info;
use pyo3_log;

#[pymodule]
fn fastnbt(py: Python, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();
    m.add_class::<PyMcWorldDescriptor>()?;
    m.add_class::<PyNbtTag>()?;
    m.add_class::<blocks::MinecraftBlock>()?;
    m.add_class::<blocks::Coordinates>()?;
    
    m.add_function(wrap_pyfunction!(load_binary, m)?)?;
    m.add_function(wrap_pyfunction!(py_log, m)?)?;

    Ok(())
}
#[pyfunction]
fn py_log(message: String)  {
    info!("{}", message);
}

#[pyfunction]
fn load_binary(input_path: String) -> PyResult<PyMcWorldDescriptor> {   
    let path_buf = PathBuf::from(input_path);
    let mc_world = McWorldDescriptor::new(path_buf)?; 
    PyMcWorldDescriptor::new(mc_world).map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(format!("{}", e)))
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PyMcWorldDescriptor {
    mc_world_descriptor: McWorldDescriptor,
    //TEST
    #[pyo3(get, set)]
    pub tag_compounds_list: Vec::<Py<PyDict>>
}

#[pymethods]
impl PyMcWorldDescriptor {
    #[new]
    pub fn new(rust_mc_world_descriptor: McWorldDescriptor) -> std::io::Result<Self> {

        let mut py_tag_list = Vec::<Py<PyDict>>::new();
        
        rust_mc_world_descriptor.tag_compounds_list.iter().for_each(|item| {
            let tag_root = nbt_tag::NbtTag::Compound(item.clone());
            py_tag_list.push(PyNbtTag::new(&tag_root).python_dict)
        });

        Ok(PyMcWorldDescriptor{ 
            mc_world_descriptor: rust_mc_world_descriptor, 
            tag_compounds_list: py_tag_list 
        })
    }

    pub fn to_json(&self, path: String) -> PyResult<()> {
        self.mc_world_descriptor.to_json(path).map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(format!("{}", e)))
    }

    pub fn get_mc_version(&self) -> String {
        self.mc_world_descriptor.get_mc_version()
    }

    pub fn search_compound(&self, key: &str) -> (bool, Vec::<Py<PyDict>>) {
        
        let mut py_tag_list = Vec::<Py<PyDict>>::new();

        let (compound_found, compound_tag_list) = self.mc_world_descriptor.search_compound(key, false);
        
        if compound_found {
            for item in compound_tag_list {
                let tag_root = nbt_tag::NbtTag::Compound(item.clone());
                py_tag_list.push(PyNbtTag::new(&tag_root).python_dict);
            }
            (true, py_tag_list)
        } else {
            (false, py_tag_list)
        }

    }

    pub fn search_blocks(&self, block_resource_location: Vec::<String>) -> HashMap::<String, Vec::<blocks::MinecraftBlock>> {
        self.mc_world_descriptor.search_blocks(block_resource_location)
    }

}

#[pyclass]
#[derive(Clone, Debug, Default)]
pub struct McWorldDescriptor {
    pub input_path: PathBuf,
    pub version: String,
    pub tag_compounds_list: Vec<nbt_tag::NbtTagCompound>,
}

impl McWorldDescriptor {
    pub fn new(input_path: PathBuf) -> std::io::Result<Self> {
        let cloned_input_path = input_path.clone();
        
        if let Ok(nbt_tag_compounds_list) = Self::read_input_path(input_path) {
            Ok(McWorldDescriptor {
                input_path: cloned_input_path,
                version: "0.0.0".to_string(),
                tag_compounds_list: nbt_tag_compounds_list,
            })
        }
        else{
            //TODO: read a file not only based on the extension, but checking the internal format
            Err(std::io::Error::new(std::io::ErrorKind::Other, "McWorldDescriptor not created because of input file error"))
        } 

        
    }

    fn read_input_path(input_path: PathBuf) -> std::io::Result<Vec<nbt_tag::NbtTagCompound>> {
        
        /* #10: The use can give in input either a folder path to Minecraft world or directly a file path.
        *  Here the path is checked, if a folder is found, the subfolder "region" is searched.
        *  If "region" is found, this is likely to be a valid Minecraft world, then the region files are read.
        */

        let mut nbt_tag_compounds_list = Vec::<nbt_tag::NbtTagCompound>::new();

        if input_path.is_dir()
        {
            /* #20: Folder path as input */
            if !input_path.exists() {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "World Directory does not exist"));
            }
    
            let region_path = input_path.join("region");
            if !region_path.exists() || !region_path.is_dir() {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "SubDir './region' does not exist"));
            }
            
            /* #30: Input path is a valid folder and contains a subfolder "region"*/
            match std::fs::read_dir(region_path) {
                Ok(entries) => {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let file_path = PathBuf::from(entry.path().to_string_lossy().into_owned());
                            nbt_tag_compounds_list.append(&mut Self::read_file_format(file_path)?);
                        }
                    }
                },
                Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Error in reading the region files")),
            }
        }
        else {
            nbt_tag_compounds_list.append(&mut Self::read_file_format(input_path)?);
        }
        

        Ok(nbt_tag_compounds_list)

    }

    fn read_file_format(input_path: PathBuf) -> std::io::Result<Vec<nbt_tag::NbtTagCompound>> {
        
        if let Some(ext) = input_path.extension().and_then(|e| e.to_str()) {

            //let mut nbt_tag_compounds_list = Vec::<nbt_tag::NbtTagCompound>::new();

            if ext == "mcr" || ext == "mca" {
                let region_file = region::RegionFile::new(input_path)?;
                let nbt_tag_compounds_list = region_file.to_compounds_list()?;
                Ok(nbt_tag_compounds_list)
            }
            else if ext == "nbt" || ext == "litematic" {
                let bin_content = generic_bin::GenericBinFile::new(input_path, generic_bin::FileType::Nbt)?;
                let nbt_tag_compounds_list = bin_content.to_compounds_list()?;
                Ok(nbt_tag_compounds_list)   
            }
            else if ext == "json" {
                let json_content = nbt_tag::NbtTagCompound::from_json(input_path)?;//Self::from_json(input_path)?;
                let mut nbt_tag_compounds_list = Vec::<nbt_tag::NbtTagCompound>::new();
                nbt_tag_compounds_list.push(json_content);
                Ok(nbt_tag_compounds_list)
            }
            else {
                Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid file extension"))
            }
        }
        else {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "File without extension"))
        }

    }

    pub fn get_mc_version(&self) -> String {
        self.version.clone()
    }

    pub fn to_json<P: AsRef<std::path::Path>>(&self, path: P) -> io::Result<()> {
        Ok(self.tag_compounds_list.get(0).unwrap().to_json(path)?)
    }

    pub fn search_blocks<'a>(&self, block_resource_location: Vec::<String>) -> HashMap::<String, Vec::<blocks::MinecraftBlock>> {
        chunk_format::inspect_chunks(block_resource_location, &self.tag_compounds_list)
    } 


    pub fn search_compound(&self, key: &str, stop_at_first: bool) ->  (bool, Vec::<&nbt_tag::NbtTagCompound>) {
        
        let mut result_list = Vec::<&nbt_tag::NbtTagCompound>::new();

        for tag_compound in self.tag_compounds_list.iter() {
            let compound_found = self.recursive_compound_search(tag_compound, &mut result_list, key, stop_at_first);
            
            if compound_found && stop_at_first {
                return (true, result_list);
            }
        }

        if result_list.is_empty() {
            return (false, result_list);
        }
        else {
            return (true, result_list);
        }
    }
        
    fn recursive_compound_search<'a>(&self, tag_compound: &'a nbt_tag::NbtTagCompound, 
                                            result_list: &mut Vec<&'a nbt_tag::NbtTagCompound>, 
                                            key: &str, 
                                            stop_at_first: bool) 
                                            -> bool {
            
        //End condition: a compound matches the key
        if tag_compound.name == key {
            result_list.push(tag_compound);
            return true;
        }
        
        //Recursion
        for (_, v) in tag_compound.values.iter() {
            if v.ty() == nbt_tag::NbtTagType::Compound {
                let compound_option = v.compound_as_ref();
                
                if let Some(compound) = compound_option {
                    let compound_found = self.recursive_compound_search(&compound, result_list, key, stop_at_first);
                    
                    if compound_found && stop_at_first {
                        return true;
                    }
                }
            }
            else if v.ty() == nbt_tag::NbtTagType::List {
                let list_option = v.list_as_ref();
                if let Some(list_option) = list_option {
                    for item in list_option.values.iter() {
                        if item.ty() == nbt_tag::NbtTagType::Compound {
                            let compound_option = item.compound_as_ref();
                            if let Some(compound) = compound_option {
                                let compound_found = self.recursive_compound_search(&compound, result_list, key, stop_at_first);
                                    if compound_found && stop_at_first {
                                        return true;
                                    } 
                            }
                        }
                        
                    }
                }
            }
        }
        
        false
    }

    /* fn read_from_binary_file(input_path: PathBuf) -> std::io::Result<Vec<nbt_tag::NbtTagCompound>> {
        if let Some(ext) = input_path.extension().and_then(|e| e.to_str()) {
            
            let mut nbt_tag_compounds_list = Vec::<nbt_tag::NbtTagCompound>::new();

            if ext == "mcr" || ext == "mca" {
                let region_file = region::RegionFile::new(input_path)?;
                nbt_tag_compounds_list = match region_file.to_compounds_list(){
                    Ok(c) => c,
                    Err(e) => return Err(e),
                }
            }
            else if ext == "nbt" || ext == "litematic" {
                let bin_content = generic_bin::GenericBinFile::new(input_path, generic_bin::FileType::Nbt)?;
                nbt_tag_compounds_list = match bin_content.to_compounds_list(){
                    Ok(c) => c,
                    Err(e) => return Err(e),
                }
            }
            Ok(nbt_tag_compounds_list)
        }
        else{
            //TODO: read a file not only based on the extension, but checking the internal format
            Err(std::io::Error::new(std::io::ErrorKind::Other, "Unsupported file type"))
        } 

        
    }*/

}


/* #[derive(Clone, Debug)]
pub struct SerializablePyDict(Py<PyDict>);

impl SerializablePyDict {
    pub fn get_py_dict(&self) -> &Py<PyDict> {
        &self.0
    }
}

impl IntoPy<Py<PyAny>> for SerializablePyDict {
    fn into_py(self, py: Python) -> Py<PyAny> {
        self.0.into_py(py)
    }
}

impl ToPyObject for SerializablePyDict {
    fn to_object(&self, py: Python) -> PyObject {
        self.0.to_object(py) // Delegate to Py<PyDict>'s implementation
    }
}

impl FromPyObject<'_> for SerializablePyDict {
    fn extract(ob: &'_ PyAny) -> PyResult<Self> {
        let py_dict: Py<PyDict> = ob.extract()?; // Extract as Py<PyDict> using PyDict's FromPyObject
        Ok(SerializablePyDict(py_dict)) // Wrap in SerializablePyDict
    }
}

impl Serialize for SerializablePyDict {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Python::with_gil(|py| {
            let dict: &PyDict = self.0.as_ref(py);
            let mut rust_dict = HashMap::new();

            for (key, value) in dict.into_iter() {
                let key_str = key.extract::<String>().map_err(serde::ser::Error::custom)?;
                let value_str = value.extract::<PyNbtTag>().map_err(serde::ser::Error::custom)?;
                rust_dict.insert(key_str, value_str);
            }

            let mut map = serializer.serialize_map(Some(rust_dict.len()))?;
            for (k, v) in rust_dict {
                map.serialize_entry(&k, &v.ser_python_dict)?;
            }
            map.end()
        })
    }
} */

#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct PyNbtTag {
    //pub nbt_tag: &'a NbtTag,
    pub python_dict: Py<PyDict>,
    //pub ser_python_dict: SerializablePyDict
}

//https://github.com/PyO3/pyo3/pull/3582 
impl PyNbtTag {

    pub fn new(nbt_tag: &nbt_tag::NbtTag) -> Self {
        let python_dict = Self::to_python_dictionary(&nbt_tag);
        //let ser_py_dict = Self::to_ser_python_dictionary(python_dict);
        Self {
            //python_dict,
            python_dict
        }
    }

    /* fn to_ser_python_dictionary(py_dict: Py<PyDict>) -> SerializablePyDict {
        SerializablePyDict(py_dict)
    } */

    fn to_python_dictionary(nbt_tag: & nbt_tag::NbtTag) -> Py<PyDict> {
        
        Python::with_gil(|py| {
            let dict: Py<PyDict> = PyDict::new(py).into();
            // TODO: Get rid of all these unwraps

            match nbt_tag.ty() {
                nbt_tag::NbtTagType::End => {

                    //let log_msg = format!("tag_end: Name: {}, Value: {}", "[END]", "[END]");
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item("END_TAG", 0).unwrap();
                    dict
                },
                nbt_tag::NbtTagType::Byte => {
                    let tag_byte = nbt_tag.byte().unwrap();

                    //let log_msg = format!("tag_byte: Name: {}, Value: {}", tag_byte.name, tag_byte.value);
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item(tag_byte.name, tag_byte.value).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::Short => {
                    let tag_short = nbt_tag.short().unwrap();

                    //let log_msg = format!("tag_short: Name: {}, Value: {}", tag_short.name, tag_short.value);
                    //crate::py_log(log_msg);


                    dict.as_ref(py).set_item(tag_short.name, tag_short.value).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::Int => {
                    let tag_int = nbt_tag.int().unwrap_or_default(); //error without default.

                    //let log_msg = format!("tag_int: Name: {}, Value: {}", tag_int.name, tag_int.value);
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item(tag_int.name, tag_int.value).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::Long => {
                    let tag_long = nbt_tag.long().unwrap();

                    //let log_msg = format!("tag_long: Name: {}, Value: {}", tag_long.name, tag_long.value);
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item(tag_long.name, tag_long.value).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::Float => {
                    let tag_float = nbt_tag.float().unwrap();

                    //let log_msg = format!("tag_float: Name: {}, Value: {}", tag_float.name, tag_float.value);
                    //crate::py_log(log_msg);


                    dict.as_ref(py).set_item(tag_float.name, tag_float.value).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::Double => {
                    let tag_double = nbt_tag.double().unwrap();

                    //let log_msg = format!("tag_double: Name: {}, Value: {}", tag_double.name, tag_double.value);
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item(tag_double.name, tag_double.value).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::ByteArray => {
                    let tag_byte_array = nbt_tag.byte_array().unwrap();

                    //let log_msg = format!("tag_byte_array: Name: {}, Value: {}", tag_byte_array.name, "[Values]");
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item(tag_byte_array.name, tag_byte_array.values).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::String => {
                    let tag_string = nbt_tag.string().unwrap();

                    //let log_msg = format!("tag_string: Name: {}, Value: {}", tag_string.name, tag_string.value);
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item(tag_string.name, tag_string.value).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::List => {
                    let tag_list = nbt_tag.list().unwrap();
                    let empty_object_array: &[PyObject] = &[];
                    let py_list: &PyList = PyList::new(py, empty_object_array);

                    //let log_msg = format!("tag_list: Name: {}, Value: {}", tag_list.name, "[NbtTagList]");
                    //crate::py_log(log_msg);

                    //not efficient, i am processind the data two times, but for now make it work
                    for list_element in &tag_list.values {
                        let py_list_element = PyNbtTag::new(list_element);
                        let _ = py_list.append(py_list_element.python_dict);

                        //let log_msg = format!("tag_list: parsed");
                        //crate::py_log(log_msg);
                    }

                    dict.as_ref(py).set_item(tag_list.name, py_list).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::Compound => {
                    let tag_compound = nbt_tag.compound().unwrap();
                    //let empty_object_array: &[PyObject] = &[];
                    let py_dict: &PyDict = PyDict::new(py);

                    //let log_msg = format!("tag_compound: Name: {}, Value: {}", tag_compound.name, "[HashMap]");
                    //crate::py_log(log_msg);

                    for (key, value) in tag_compound.values.iter() {
                        let py_tag = PyNbtTag::new(value);
                        let _ = py_dict.set_item(key, py_tag.python_dict);

                        //let log_msg = format!("tag_compound_hashmap: Name: {}, Value: {}", key, "[NbtTag]");
                        //crate::py_log(log_msg);
                    }

                    dict.as_ref(py).set_item(tag_compound.name, py_dict).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::IntArray => {
                    let tag_int_array = nbt_tag.int_array().unwrap();

                    //let log_msg = format!("tag_int_array: Name: {}, Value: {}", tag_int_array.name, "[Values]");
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item(tag_int_array.name, tag_int_array.values).unwrap();
                    dict

                },
                nbt_tag::NbtTagType::LongArray => {
                    let tag_long_array = nbt_tag.long_array().unwrap();

                    //let log_msg = format!("tag_long_array: Name: {}, Value: {}", tag_long_array.name, "[Values]");
                    //crate::py_log(log_msg);

                    dict.as_ref(py).set_item(tag_long_array.name, tag_long_array.values).unwrap();
                    dict

                }
            }
        })
    }
}
