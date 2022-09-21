use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use crate::package::mutation_steps::utils::*;
use crate::package::mutation_steps::combination::*;
use crate::package::mutation_steps::rep::*;
use move_ir_types::location::*;

// Firstly create all the rep and comb cases
// ( So that there is no need to put the mutation type logic here


//Temporarily insert rep case at the end of each combination
pub fn run_mix_generation() -> anyhow::Result<()> {

    //evolution_vec stores all the rep combination

    let mut evolution_vec: Vec<Vec<Option<Loc>>> = Vec::new();
    for i in 0..evolution_round {
        // for each evolution_round, firstly generate rep and comb cases
        let evolution_rep_status =  rep_set_generation(i);
        let mut evolution_combination_status = normal_set_generation(i);

        // This vector contains the loc that has already been repeated and inserted
        let mut reped_vec: Vec<Option<Loc>> = Vec::new();

        // construct the evolution_vec
        for (func, rep) in evolution_rep_status{
            for single_vec in rep{
                if single_vec.len() != i+2{
                    continue;
                }
                evolution_vec.push(single_vec);
            }
        };


        // iterate through the comb map and insert rep at the according location

        for (func, mut com_loc) in evolution_combination_status.clone(){

            for single_loc in com_loc.clone(){

                if single_loc.len() != i+2{
                    continue;
                }

                // Create rep to be compared with
                // If rep can exist, insert it at the end of the combination
                let mut rep_temp: Vec<Option<Loc>> = Vec::new();
                for round in 0..(i.clone() + 2) {
                    rep_temp.push(single_loc[0].clone());
                }

                if evolution_vec.contains(&rep_temp) && !reped_vec.contains(&single_loc[0]){
                    let pos = com_loc.clone().binary_search(&single_loc).unwrap_or_else(|e| e);
                    com_loc.insert(pos, rep_temp.clone());
                    reped_vec.push(single_loc[0]);
                    evolution_combination_status.insert(func.clone(), com_loc.clone());

                }

            }
        };
        // write this into the status file
        fs::remove_file(&evolution_status_file_path);
        write_pretty_json(&evolution_status_file_path, evolution_combination_status);

    }



    Ok(())
}