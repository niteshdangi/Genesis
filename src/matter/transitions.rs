use std::{collections::{BTreeMap,BTreeSet},sync::Arc};
use rayon::prelude::*;
use crate::{chemistry::seeded_value,law_system::SimulationError};
use super::{MatterLaw,MatterState,MatterEvent,state::{EventKind,Particle,SiteSet}};

#[derive(Clone)]
enum Action {
    Move(Vec<(u64,u32)>), Rotate(u64,u8), Bind(u64,u64,Option<[u64;2]>), Unbind(u64,u64),
    Switch(u64), Charge(u64), Dissipate(u64), Transfer(u64,u64),
}
struct Proposal { key:(u64,u64,u64,u8), particles:Vec<u64>, sites:Vec<u32>, action:Action }

fn draw(law:&MatterLaw,tick:u64,stage:u64,id:u64,salt:u64) -> u64 { seeded_value(law.config.seed,tick,stage,id,salt) }
fn happens(law:&MatterLaw,tick:u64,stage:u64,id:u64,salt:u64,rate:u32) -> bool {
    u128::from(draw(law,tick,stage,id,salt))*1_000_000 < u128::from(rate)<<64
}
fn event(tick:u64,kind:EventKind,particles:[u64;2],positions:[u32;2],before:[u32;2],after:[u32;2],energy:u32) -> MatterEvent {
    MatterEvent { tick,kind,particles,positions,before,after,energy }
}
fn proposal(law:&MatterLaw,tick:u64,stage:u64,a:u64,b:u64,tag:u8,mut particles:Vec<u64>,mut sites:Vec<u32>,action:Action) -> Proposal {
    particles.sort_unstable(); particles.dedup(); sites.sort_unstable(); sites.dedup();
    Proposal { key:(draw(law,tick,stage,a,b^u64::from(tag)),a,b,tag),particles,sites,action }
}
fn collect<F>(law:&MatterLaw,count:usize,evaluate:F) -> Vec<Proposal>
where F:Fn(u64)->Vec<Proposal>+Sync+Send {
    let groups:Vec<Vec<Proposal>>=if law.config.execution.parallel {
        (1..=count as u64).into_par_iter().map(&evaluate).collect()
    } else { (1..=count as u64).map(evaluate).collect() };
    groups.into_iter().flatten().collect()
}
fn resolve(mut proposals:Vec<Proposal>) -> Vec<Action> {
    proposals.sort_by_key(|p|p.key);
    let mut particles=BTreeSet::new(); let mut sites=BTreeSet::new(); let mut actions=Vec::new();
    for p in proposals {
        if p.particles.iter().any(|id|particles.contains(id)) || p.sites.iter().any(|site|sites.contains(site)) { continue; }
        particles.extend(p.particles); sites.extend(p.sites); actions.push(p.action);
    }
    actions
}

pub fn reservoirs(law:&MatterLaw,current:&MatterState,next:&mut MatterState,tick:u64) -> Result<(),SimulationError> {
    next.events.clear(); next.events_json.clear(); next.events_truncated=false;
    if !law.config.reservoir.enabled { return Ok(()); }
    for y in 0..current.space.height { for x in 0..current.space.width {
        let source=current.space.position([x,y,0]); let sink=current.space.position([x,y,current.space.depth-1]);
        let before=current.heat(source);
        let amount=law.config.reservoir.input_per_site.min(law.config.reservoir.capacity.saturating_sub(before));
        next.add_heat(source,amount)?;
        next.energy_input=next.energy_input.checked_add(u128::from(amount)).ok_or_else(||SimulationError::new("reservoir input overflow"))?;
        if amount>0 { next.event(event(tick,EventKind::Input,[0,0],[source,source],[before,0],[before+amount,0],amount),&law.config)?; }
        let before=current.heat(sink); let amount=before.min(law.config.reservoir.sink_per_site);
        next.spend_heat(sink,amount)?;
        next.energy_output=next.energy_output.checked_add(u128::from(amount)).ok_or_else(||SimulationError::new("reservoir output overflow"))?;
        if amount>0 { next.event(event(tick,EventKind::Output,[0,0],[sink,sink],[before,0],[before-amount,0],amount),&law.config)?; }
    } }
    Ok(())
}

pub fn heat(law:&MatterLaw,current:&MatterState,next:&mut MatterState,tick:u64) -> Result<(),SimulationError> {
    let mut flux=Vec::new();
    let mut candidates=(*current.heat_active).clone();candidates.union_with(&current.heat_dirty);
    let dense=candidates.len()>current.space.len()/16;
    let mut outgoing=if dense{vec![0u8;current.space.len()]}else{Vec::new()};
    let mut incoming=if dense{vec![0u8;current.space.len()]}else{Vec::new()};
    let mut sparse=BTreeMap::<u32,(u8,u8)>::new();
    let mut active=SiteSet::default();
    next.heat_dirty=Arc::new(SiteSet::default());
    let available_events=law.config.execution.event_limit.saturating_sub(next.events.len());
    for source in candidates.iter() {
            let energy=current.heat(source);
            if energy<2 { continue; }
            let mut budget=energy;
            for direction in 0..6 {
                let target=current.space.neighbor(source,direction);
                if budget>0 && energy>current.heat(target).saturating_add(1) {
                    budget-=1;active.insert(source);
                    if dense{outgoing[source as usize]+=1;incoming[target as usize]+=1;}
                    else{sparse.entry(source).or_default().0+=1;sparse.entry(target).or_default().1+=1;}
                    if flux.len()<available_events {flux.push((source,target));}
                    else if law.config.execution.full_provenance {return Err(SimulationError::new("heat provenance exceeds the event budget; generation not committed"));}
                    else {next.events_truncated=true;}
                }
            }
    }
    next.heat_active=Arc::new(active);
    if let Some(gpu)=&law.gpu {
        let energies=(0..current.space.len() as u32).map(|p|current.heat(p)).collect::<Vec<_>>();
        let result=gpu.diffuse(&energies,current.space)?;
        for (position,&energy) in result.iter().enumerate() { if current.heat(position as u32)!=energy { next.set_heat(position as u32,energy); } }
    } else if dense {
        for position in 0..current.space.len() as u32 {
            if outgoing[position as usize]==0 && incoming[position as usize]==0 {continue;}
            let energy=current.heat(position)-u32::from(outgoing[position as usize]);
            let energy=energy.checked_add(u32::from(incoming[position as usize])).ok_or_else(||SimulationError::new("heat transport overflow"))?;
            next.set_heat(position,energy);
        }
    } else {
        for (position,(outgoing,incoming)) in sparse {
            let energy=(current.heat(position)-u32::from(outgoing)).checked_add(u32::from(incoming))
                .ok_or_else(||SimulationError::new("heat transport overflow"))?;
            next.set_heat(position,energy);
        }
    }
    // Flux events describe simultaneous snapshot transfers, not sequential re-spending.
    for (source,target) in flux {
        next.event(event(tick,EventKind::Transfer,[0,0],[source,target],[current.heat(source),current.heat(target)],
            [next.heat(source),next.heat(target)],1),&law.config)?;
    }
    Ok(())
}

fn move_allowed(law:&MatterLaw,state:&MatterState,changes:&[(u64,u32)]) -> bool {
    let ids:BTreeSet<_>=changes.iter().map(|&(id,_)|id).collect();
    let targets:BTreeSet<_>=changes.iter().map(|&(_,p)|p).collect();
    if targets.len()!=changes.len() { return false; }
    for &(id,target) in changes {
        let Some(p)=state.particle(id) else { return false; };
        if state.occupant(target)!=0 && !ids.contains(&state.occupant(target)) { return false; }
        for &other in &p.bonds {
            if other==0 { continue; }
            let Some(q)=state.particle(other) else { return false; };
            let q_target=changes.iter().find(|&&(member,_)|member==other).map_or(q.position,|&(_,position)|position);
            let Some(direction)=state.space.direction(target,q_target) else { return false; };
            if !MatterState::port_open(p,direction,&law.config) || !MatterState::port_open(q,direction^1,&law.config) { return false; }
        }
    }
    true
}

pub fn motion(law:&MatterLaw,current:&MatterState,next:&mut MatterState,tick:u64) -> Result<(),SimulationError> {
    let proposals=collect(law,current.count(),|id| {
        let Some(p)=current.particle(id) else { return Vec::new(); };
        if p.excitation==0 || !happens(law,tick,110,id,0,law.config.motion_rate) { return Vec::new(); }
        let mut offsets=Vec::new();
        for x in -1..=1 { for y in -1..=1 { for z in -1..=1 { if [x,y,z]!=[0,0,0] { offsets.push([x,y,z]); } } } }
        offsets.sort_by_key(|offset|draw(law,tick,110,id,((offset[0]+1)*9+(offset[1]+1)*3+offset[2]+1) as u64+1));
        let mut result=Vec::new();
        for offset in offsets {
            let target=current.space.shift(p.position,offset); let changes=vec![(id,target)];
            if move_allowed(law,current,&changes) {
                let members=std::iter::once(id).chain(p.bonds.into_iter().filter(|&id|id!=0)).collect();
                result.push(proposal(law,tick,110,id,target as u64,0,members,vec![p.position,target],Action::Move(changes))); break;
            }
        }
        if let Some(other)=p.bonds.into_iter().find(|&other|other>id) {
            if let Some(q)=current.particle(other) {
                let direction=(draw(law,tick,110,id,999)%6) as usize;
                let changes=vec![(id,current.space.neighbor(p.position,direction)),(other,current.space.neighbor(q.position,direction))];
                if move_allowed(law,current,&changes) {
                    let members=std::iter::once(id).chain(std::iter::once(other)).chain(p.bonds).chain(q.bonds).filter(|&id|id!=0).collect();
                    let sites=vec![p.position,q.position,changes[0].1,changes[1].1];
                    result.push(proposal(law,tick,110,id,other,1,members,sites,Action::Move(changes)));
                }
            }
        }
        if p.degree()==0 { result.push(proposal(law,tick,110,id,0,2,vec![id],vec![p.position],Action::Rotate(id,(p.orientation+1)%3))); }
        result
    });
    for action in resolve(proposals) {
        match action {
            Action::Move(changes) => {
                let mut originals=Vec::new();
                for &(id,_) in &changes { originals.push(current.particle(id).ok_or_else(||SimulationError::new("missing motion source"))?); }
                for p in &originals { next.set_occupant(p.position,0); }
                for &(id,target) in &changes {
                    let mut p=next.particle(id).ok_or_else(||SimulationError::new("missing motion particle"))?;
                    p.position=target; p.bonds=[0;6]; next.write_particle(p)?; next.set_occupant(target,id);
                }
                for old in &originals {
                    let mut p=next.particle(old.id).ok_or_else(||SimulationError::new("missing moved particle"))?;
                    for &other in &old.bonds {
                        if other==0 { continue; }
                        let mut q=next.particle(other).ok_or_else(||SimulationError::new("missing bonded neighbor"))?;
                        let direction=next.space.direction(p.position,q.position).ok_or_else(||SimulationError::new("motion breaks geometry"))?;
                        p.bonds[direction]=other;
                        for bond in &mut q.bonds { if *bond==p.id { *bond=0; } }
                        q.bonds[direction^1]=p.id; next.write_particle(q)?;
                    }
                    next.write_particle(p)?;
                }
                let source=originals[0]; let mut p=next.particle(source.id).ok_or_else(||SimulationError::new("missing motion payer"))?;
                p.excitation-=1; next.write_particle(p)?; next.add_heat(source.position,1)?;
                for old in originals {
                    let target=next.particle(old.id).ok_or_else(||SimulationError::new("missing moved particle"))?.position;
                    next.event(event(tick,EventKind::Move,[old.id,0],[old.position,target],[old.position,0],[target,0],u32::from(old.id==source.id)),&law.config)?;
                }
            }
            Action::Rotate(id,orientation) => {
                let mut p=next.particle(id).ok_or_else(||SimulationError::new("missing rotating particle"))?;
                let before=p.orientation; p.orientation=orientation; p.excitation-=1; next.write_particle(p)?; next.add_heat(p.position,1)?;
                next.event(event(tick,EventKind::Rotate,[id,0],[p.position,p.position],[before as u32,0],[orientation as u32,0],1),&law.config)?;
            }
            _ => return Err(SimulationError::new("invalid motion action")),
        }
    }
    Ok(())
}

fn environment(law:&MatterLaw,state:&MatterState,p:Particle,excluded:&[u64]) -> (u32,u32,u32) {
    let mut active=0; let mut inactive=0; let mut strength=0u32;
    for direction in 0..6 {
        let id=state.occupant(state.space.neighbor(p.position,direction));
        if excluded.contains(&id) { continue; }
        if let Some(q)=state.particle(id) {
            if q.conformation==1 { active+=1; strength=strength.saturating_add(law.config.materials[q.material as usize].catalytic_strength); }
            else { inactive+=1; }
        }
    }
    (active,inactive,strength)
}

fn contact_source(law:&MatterLaw,state:&MatterState,p:Particle,q:Particle,direction:usize)->Option<([u64;2],u32,u32)> {
    if law.config.law_version==1 {return None;}
    let mut candidates=Vec::new();
    for offset in 0..6 {
        if offset/2==direction/2 {continue;}
        let Some(a)=state.particle(state.occupant(state.space.neighbor(p.position,offset))) else{continue;};
        let Some(b)=state.particle(state.occupant(state.space.neighbor(q.position,offset))) else{continue;};
        if a.id==p.id || a.id==q.id || b.id==p.id || b.id==q.id || a.id==b.id
            || a.conformation!=1 || b.conformation!=1
            || a.bonds[direction]!=b.id || b.bonds[direction^1]!=a.id {continue;}
        let mismatches=u32::from(a.material!=p.material)+u32::from(b.material!=q.material);
        if mismatches>u32::from(law.config.law_version==3) {continue;}
        let divisor=if mismatches==0 {1} else {16};
        let strength=law.config.materials[a.material as usize].catalytic_strength
            .saturating_add(law.config.materials[b.material as usize].catalytic_strength).min(4);
        if strength>0 {candidates.push(([a.id,b.id],strength,divisor));}
    }
    candidates.sort_by_key(|&(ids,strength,divisor)|(std::cmp::Reverse(strength*16/divisor),ids));
    candidates.into_iter().next()
}

pub fn interact(law:&MatterLaw,current:&MatterState,next:&mut MatterState,tick:u64) -> Result<(),SimulationError> {
    let proposals=collect(law,current.count(),|id| {
        let Some(p)=current.particle(id) else { return Vec::new(); };
        let material=&law.config.materials[p.material as usize];
        let (active,inactive,catalysis)=environment(law,current,p,&[]);
        let barrier=material.barrier.saturating_sub(catalysis);
        let influence=if p.conformation==0 { active } else { inactive };
        let rate=material.switching_rate.saturating_add(material.coupling.saturating_mul(influence)).min(1_000_000);
        let mut proposals=Vec::new();
        let switch_cost=if p.conformation==0 { material.conformation_energy.saturating_add(1) } else { 1 };
        if p.excitation>=switch_cost.max(barrier) && happens(law,tick,120,id,0,rate) {
            proposals.push(proposal(law,tick,120,id,0,0,vec![id],vec![p.position],Action::Switch(id)));
        }
        if current.heat(p.position)>p.excitation.saturating_add(1) && p.excitation<u32::MAX && happens(law,tick,120,id,1,law.config.exchange_rate) {
            proposals.push(proposal(law,tick,120,id,0,1,vec![id],vec![p.position],Action::Charge(id)));
        }
        if p.excitation>current.heat(p.position).saturating_add(1) && happens(law,tick,120,id,2,law.config.exchange_rate) {
            proposals.push(proposal(law,tick,120,id,0,2,vec![id],vec![p.position],Action::Dissipate(id)));
        }
        for direction in 0..6 {
            let other=current.occupant(current.space.neighbor(p.position,direction));
            if other<=id { continue; }
            let Some(q)=current.particle(other) else { continue; };
            let (_,_,other_catalysis)=environment(law,current,q,&[id]);
            let (_,_,first_catalysis)=environment(law,current,p,&[other]);
            let barrier=material.barrier.max(law.config.materials[q.material as usize].barrier).saturating_sub(first_catalysis.saturating_add(other_catalysis));
            let energy=u64::from(p.excitation)+u64::from(q.excitation);
            if p.bonds[direction]==other {
                if energy>=u64::from(barrier.max(1)) && happens(law,tick,120,id,other^3,law.config.breaking_rate) {
                    proposals.push(proposal(law,tick,120,id,other,3,vec![id,other],vec![p.position,q.position],Action::Unbind(id,other)));
                }
            } else if p.degree()<material.valence as usize && q.degree()<law.config.materials[q.material as usize].valence as usize
                && MatterState::port_open(p,direction,&law.config) && MatterState::port_open(q,direction^1,&law.config)
                && energy>=u64::from(barrier.max(law.config.bond_energy+1)) {
                let affinity=u64::from(material.affinities[q.material as usize]);
                let ordinary=(u64::from(law.config.binding_rate)*affinity/100) as u32;
                let contact=contact_source(law,current,p,q,direction);
                let bonus=contact.map_or(0,|(_,strength,divisor)|u64::from(strength)*125_000*affinity/100/u64::from(divisor));
                let rate=(u64::from(ordinary)+bonus).min(1_000_000) as u32;
                if happens(law,tick,120,id,other^4,rate) {
                    let sources=if happens(law,tick,120,id,other^4,ordinary) {None} else {contact.map(|(ids,_,_)|ids)};
                    let mut particles=vec![id,other];let mut sites=vec![p.position,q.position];
                    if let Some(sources)=sources {
                        particles.extend(sources);
                        for source in sources {if let Some(catalyst)=current.particle(source){sites.push(catalyst.position);}}
                    }
                    proposals.push(proposal(law,tick,120,id,other,4,particles,sites,Action::Bind(id,other,sources)));
                }
            }
            if p.bonds[direction]==other && p.excitation!=q.excitation && happens(law,tick,120,id,other^5,law.config.exchange_rate) {
                let (a,b)=if p.excitation>q.excitation { (p,q) } else { (q,p) };
                if a.excitation>b.excitation.saturating_add(1) { proposals.push(proposal(law,tick,120,id,other,5,vec![id,other],vec![p.position,q.position],Action::Transfer(a.id,b.id))); }
            }
        }
        proposals
    });
    for action in resolve(proposals) {
        match action {
            Action::Switch(id) => {
                let mut p=next.particle(id).ok_or_else(||SimulationError::new("missing switching particle"))?;
                let before=p.conformation; let energy=law.config.materials[p.material as usize].conformation_energy;
                if before==0 { p.excitation-=energy+1; next.add_heat(p.position,1)?; }
                else { p.excitation-=1; next.add_heat(p.position,energy+1)?; }
                p.conformation=1-before; next.write_particle(p)?;
                next.event(event(tick,EventKind::Switch,[id,0],[p.position,p.position],[before as u32,0],[p.conformation as u32,0],energy),&law.config)?;
            }
            Action::Charge(id) | Action::Dissipate(id) => {
                let charge=matches!(action,Action::Charge(_));
                let mut p=next.particle(id).ok_or_else(||SimulationError::new("missing energy-exchanging particle"))?;
                let before=p.excitation;
                if charge { next.spend_heat(p.position,1)?; p.excitation+=1; } else { p.excitation-=1; next.add_heat(p.position,1)?; }
                next.write_particle(p)?;
                next.event(event(tick,if charge {EventKind::Charge} else {EventKind::Dissipate},[id,0],[p.position,p.position],[before,0],[p.excitation,0],1),&law.config)?;
            }
            Action::Bind(a,b,_) | Action::Unbind(a,b) => {
                let bind=matches!(action,Action::Bind(..));
                let sources=if let Action::Bind(_,_,sources)=action {sources} else {None};
                let mut p=next.particle(a).ok_or_else(||SimulationError::new("missing reaction endpoint"))?;
                let mut q=next.particle(b).ok_or_else(||SimulationError::new("missing reaction endpoint"))?;
                let direction=next.space.direction(p.position,q.position).ok_or_else(||SimulationError::new("nonlocal reaction"))?;
                let cost=if bind {law.config.bond_energy+1} else {1};
                let first=p.excitation.min(cost); p.excitation-=first; q.excitation-=cost-first;
                if bind { p.bonds[direction]=b; q.bonds[direction^1]=a; next.add_heat(p.position,1)?; }
                else { p.bonds[direction]=0; q.bonds[direction^1]=0; next.add_heat(p.position,law.config.bond_energy+1)?; }
                next.write_particle(p)?; next.write_particle(q)?;
                next.event(event(tick,if bind {EventKind::Bind} else {EventKind::Unbind},[a,b],[p.position,q.position],[u32::from(!bind),0],[u32::from(bind),0],law.config.bond_energy),&law.config)?;
                if let Some(sources)=sources {
                    for (source,target) in sources.into_iter().zip([p,q]) {
                        let catalyst=current.particle(source).ok_or_else(||SimulationError::new("missing contact catalyst"))?;
                        let states=[u32::from(catalyst.conformation),u32::from(target.conformation)];
                        next.event(event(tick,EventKind::ContactCatalysis,[source,target.id],
                            [catalyst.position,target.position],states,states,0),&law.config)?;
                    }
                }
            }
            Action::Transfer(a,b) => {
                let mut p=next.particle(a).ok_or_else(||SimulationError::new("missing excitation source"))?;
                let mut q=next.particle(b).ok_or_else(||SimulationError::new("missing excitation target"))?;
                let before=[p.excitation,q.excitation]; p.excitation-=1; q.excitation=q.excitation.checked_add(1).ok_or_else(||SimulationError::new("excitation overflow"))?;
                next.write_particle(p)?; next.write_particle(q)?;
                next.event(event(tick,EventKind::Transfer,[a,b],[p.position,q.position],before,[p.excitation,q.excitation],1),&law.config)?;
            }
            _ => return Err(SimulationError::new("invalid material action")),
        }
    }
    next.refresh_events()?;
    Ok(())
}
