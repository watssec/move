use crate::package::mutation::*;
use crate::package::mutation_steps::utils::*;
use move_compiler::parser::ast::FunctionName;
use move_compiler::shared::Identifier;
use move_ir_types::location::*;
use std::collections::HashSet;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::fs::{self, OpenOptions};
use std::iter::FromIterator;
use std::path::Path;
use pbr::ProgressBar;

pub fn run_normal_set_generation() -> anyhow::Result<()> {
    for i in 0..evolution_round {
        let evolution_status = normal_set_generation(i);
        fs::remove_file(&evolution_status_file_path);
        let mut evolution_status_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&evolution_status_file_path)
            .unwrap();
        let pretty_evolution_status = serde_json::to_value(&evolution_status).unwrap();
        serde_json::to_writer_pretty(&evolution_status_file, &pretty_evolution_status)?;
    }
    Ok(())
}
pub fn normal_set_generation(round_id: usize)
    -> BTreeMap<String, Vec<Vec<Option<Loc>>>> {
    let mut mutated_loc_file = if Path::new(&mutated_file_path).exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&mutated_file_path)
            .unwrap()
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .create(true)
            .open(&mutated_file_path)
            .unwrap()
    };
    let mut mutate_loc_original: Vec<Loc> = Vec::new();

    match serde_json::from_reader(&mutated_loc_file) {
        Ok(content) => mutate_loc_original = content,
        Err(e) => {}
    }

    let mut evolution_status_file = if Path::new(&evolution_status_file_path).exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&evolution_status_file_path)
            .unwrap()
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .create(true)
            .open(&evolution_status_file_path)
            .unwrap()
    };
    let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

    match serde_json::from_reader(&evolution_status_file) {
        Ok(content) => evolution_status = content,
        Err(e) => {}
    }

    let mut env_function_map_file = if Path::new(&env_function_map_file_path).exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&env_function_map_file_path)
            .unwrap()
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .create(true)
            .open(&env_function_map_file_path)
            .unwrap()
    };
    let mut env_function_map_keys_file = if Path::new(&env_function_map_keys_file_path).exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&env_function_map_keys_file_path)
            .unwrap()
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .create(true)
            .open(&env_function_map_keys_file_path)
            .unwrap()
    };
    let mut function_map: BTreeMap<Loc, Option<FunctionName>> = BTreeMap::new();
    let mut function_map_value: Vec<Option<FunctionName>> = Vec::new();
    match serde_json::from_reader(&env_function_map_file) {
        Ok(content) => function_map_value = content,
        Err(e) => {}
    }
    let mut function_map_keys: Vec<Loc> = Vec::new();
    match serde_json::from_reader(&env_function_map_keys_file) {
        Ok(content) => function_map_keys = content,
        Err(e) => {}
    }
    let mut cnt = 0;
    for i in function_map_keys{
        function_map.insert(i, function_map_value[cnt]);
        cnt = cnt + 1;
    }

    let function_keys: Vec<String> = evolution_status.clone().into_keys().collect();
    // iterate through the functions
    let mut bar_length = function_keys.len();
    let mut pb = ProgressBar::new(bar_length.try_into().unwrap());
    pb.format("╢▌▌░╟");
    for key in function_keys {
        let str_key = key.as_str();
        // initialize the new evolution set
        let mut new_vec: Vec<Vec<Option<Loc>>> = Vec::new();
        // for every set in the vector
        // if this vec is labelled FIN skip it
        // TODO: If 3rd+ evolution need to be supported, this continue condition
        // should be changed to !=
        for vec in evolution_status.get(str_key).unwrap().clone() {
            // check here
            let mut fin_flag = check_fin(vec.to_owned());

            // push vec into new_vec
            new_vec.push(vec.to_owned().clone());
            if fin_flag {
                continue;
            }
            if vec.len() < round_id + 1 {
                continue;
            }

            // this brings repetition
            // step0: turn vec into set
            let mut current_set = HashSet::new();
            for item in vec.clone() {
                current_set.insert(item);
            }
            // step 0.5 create a hashset for mutate_loc_original
            let mut mutate_loc_original_set = HashSet::new();
            for item in mutate_loc_original.clone() {
                // add a condition -> under the same function
                let item_function_name: String = function_map
                    .get(&item)
                    .unwrap()
                    .unwrap()
                    .value()
                    .as_str()
                    .to_owned();
                if item_function_name == key {
                    if !check_fin(vec![Some(item.clone())]) {
                        mutate_loc_original_set.insert(Some(item));
                    }
                }
            }
            // step1: get a sub hashset

            let mut sub_set: HashSet<Option<Loc>> = &mutate_loc_original_set - &current_set;

            // step2: append one item from the sub hashset into
            for add in sub_set {
                let mut current_vec = vec.clone();
                current_vec.push(add);
                new_vec.push(current_vec);
            }
        }

        // prune when subset
        let mut retain_list = Vec::new();
        for i in 0..new_vec.len() {
            retain_list.push(true);
        }

        let mut repetition = Vec::new();
        let mut repetition_set = Vec::new();
        for item_outer in new_vec.clone() {
            for item_inner in new_vec.clone() {
                let item_outer_set: HashSet<Option<Loc>> = HashSet::from_iter(item_outer.clone());
                let item_inner_set: HashSet<Option<Loc>> = HashSet::from_iter(item_inner.clone());
                let index_outer = new_vec.iter().position(|x| *x == item_outer).unwrap();
                let index_inner = new_vec.iter().position(|x| *x == item_inner).unwrap();
                //Set<a,b> and Set<b,a> are still considered as two vecs..
                if item_inner_set.is_subset(&item_outer_set) && item_inner_set != item_inner_set {
                    let index_outer = new_vec.iter().position(|x| *x == item_outer).unwrap();
                    retain_list[index_outer] = false;
                }

                //Two situations -> the same item/ set-wise same item
                if item_inner_set == item_outer_set {
                    // not the same item
                    if index_outer != index_inner {
                        let mut temp_set = HashSet::new();
                        temp_set.insert(index_inner);
                        temp_set.insert(index_outer);

                        if !repetition_set.contains(&temp_set) {
                            repetition.push(vec![index_inner, index_outer]);
                            repetition_set.push(temp_set);
                        };
                    }
                }
            }
        }
        for item in repetition {
            retain_list[item[0]] = false;
        }
        let mut iter = retain_list.iter();
        new_vec.retain(|_| *iter.next().unwrap());
        evolution_status.insert(key.to_string(), new_vec.clone());
        pb.inc();
    }
    evolution_status
}
