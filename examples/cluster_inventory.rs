use std::{collections::BTreeSet,path::PathBuf};
use genesis::{persistence::{read_json,write_new_json},universe::Universe};
use serde_json::{Value,json};

fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut args=std::env::args().skip(1);
    let source=PathBuf::from(args.next().ok_or("source campaign required")?);
    let output=PathBuf::from(args.next().ok_or("new output JSON required")?);
    if args.next().is_some(){return Err("unexpected argument".into());}
    let protocol:Value=read_json(&source.join("protocol.json"))?;
    let seeds:Vec<u64>=serde_json::from_value(protocol["seeds"].clone())?;let mut rows=Vec::new();
    for seed in seeds {
        let seed_path=source.join(format!("seed-{seed}"));let cohorts:Vec<Vec<u64>>=read_json(&seed_path.join("selected-clusters.json"))?;
        let base=Universe::load_checkpoint(&seed_path.join("base.gns"))?;
        for cohort in cohorts {
            let root=*cohort.first().ok_or("empty cohort")?;let path=seed_path.join(format!("cluster-{root}"));
            let trained=Universe::load_checkpoint(&path.join("conditioned/final.gns"))?;
            let reference=Universe::load_checkpoint(&path.join("unconditioned/final.gns"))?;
            let mut snapshots=Vec::new();let mut added=BTreeSet::new();let mut lost=BTreeSet::new();let mut neighbors=BTreeSet::new();
            for &id in &cohort {
                let a=trained.material_state().ok_or("missing trained matter")?;let b=reference.material_state().ok_or("missing reference matter")?;
                let p=a.particle(id).ok_or("missing trained particle")?;let q=b.particle(id).ok_or("missing reference particle")?;
                let initial=base.material_state().ok_or("missing base matter")?.particle(id).ok_or("missing initial particle")?;
                for other in p.bonds.into_iter().filter(|&n|n!=0) {if !q.bonds.contains(&other) && !initial.bonds.contains(&other){added.insert((id.min(other),id.max(other)));}}
                for other in q.bonds.into_iter().filter(|&n|n!=0) {if !p.bonds.contains(&other) && initial.bonds.contains(&other){lost.insert((id.min(other),id.max(other)));}}
                for (state,position) in [(a,p.position),(b,q.position)] {for d in 0..6 {let other=state.occupant(state.space.neighbor(position,d));if other!=0 && !cohort.contains(&other){neighbors.insert(other);}}}
                snapshots.push(json!({"id":id,"material":p.material,
                    "trained":{"xyz":a.space.xyz(p.position),"orientation":p.orientation,"conformation":p.conformation,"bonds":p.bonds},
                    "reference":{"xyz":b.space.xyz(q.position),"orientation":q.orientation,"conformation":q.conformation,"bonds":q.bonds}}));
            }
            let outside:Vec<_>=neighbors.into_iter().map(|id| {
                let p=trained.material_state().and_then(|s|s.particle(id));let q=reference.material_state().and_then(|s|s.particle(id));
                json!({"id":id,"trained_conformation":p.map(|p|p.conformation),"reference_conformation":q.map(|q|q.conformation)})
            }).collect();
            rows.push(json!({"seed":seed,"root":root,"cohort":cohort,"particles":snapshots,"added_links":added,"lost_links":lost,"adjacent_outside_particles":outside}));
        }
    }
    write_new_json(&output,&json!({"source":source.to_string_lossy(),"clusters":rows}))?;Ok(())
}
