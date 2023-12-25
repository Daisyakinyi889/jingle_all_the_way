#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};
use std::cmp::Reverse;


type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Gift {
    id: u64,
    name: String,
    recipient_id: u64, // referencing Recipient struct
    description: String,
    status: String,
    created_at: u64,
    updated_at: Option<u64>,
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Recipient {
    id: u64,
    name: String,
    relationship: String, // e.g. "Brother", "Grandmother", "Best Friend"
    created_at: u64,
    updated_at: Option<u64>,
}

// Implement Storable and BoundedStorable traits for Gift
impl Storable for Gift {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Gift {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}


// Implement Storable and BoundedStorable traits for Recipient
impl Storable for Recipient {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Recipient {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}



thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );
    static GIFT_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );   
    static RECIPIENT_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))), 0)
            .expect("Cannot create a counter")
    ); 
    static GIFT_STORAGE: RefCell<StableBTreeMap<u64, Gift, Memory>> =
    RefCell::new(StableBTreeMap::init(
        MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2)))
    ));
    static RECIPIENT_STORAGE: RefCell<StableBTreeMap<u64, Recipient, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(3)))
    ));


}


#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct GiftPayload {
    name: String,
    recipient_id: u64, // referencing Recipient struct
    description: String,
    status: String,
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct  RecipientPayload {
    name: String,
    relationship: String,
}


// Implementation of the CRUD structure 

// get all Gifts 
#[ic_cdk::query]
fn get_all_gifts() -> Result<Vec<Gift>, Error>{
    let gifts_map: Vec<(u64,Gift)> = GIFT_STORAGE.with(|service| service.borrow().iter().collect());
    let gifts: Vec<Gift> = gifts_map.into_iter().map(|(_, gift)| gift).collect();

    if !gifts.is_empty(){
        Ok(gifts)
    }else {
        Err(Error::NotFound{
            msg: "No Gifts found.".to_string(),
        })
    }
}

// get a Gift by Id 
#[ic_cdk::query]
fn get_gift(id: u64) -> Result<Gift, Error> {
    match _get_gift(&id) {
        Some(gift) => Ok(gift),
        None => Err(Error::NotFound {
            msg: format!("Gift with id={} not found", id),
        }),
    }
}
// a helper method to get a Gift by id.
fn _get_gift(id: &u64) -> Option<Gift> {
    GIFT_STORAGE.with(|service| service.borrow().get(id))
}

// Create a New Gift

#[ic_cdk::update]
fn add_gift(payload: GiftPayload) -> Option<Gift> {
    //Input validation
    if payload.name.is_empty() || payload.description.is_empty() || payload.status.is_empty() {
        return None;
    }
    let id = GIFT_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");
    let new_gift = Gift{ 
        id, 
        name: payload.name,
        recipient_id: payload.recipient_id, // referencing Recipient struct
        description: payload.description,
        status: payload.status,
        created_at: time(), 
        updated_at: None 
    };
    do_insert_gift(&new_gift);
    Some(new_gift)
}


fn do_insert_gift(gift: &Gift) {
    GIFT_STORAGE.with(|service| service.borrow_mut().insert(gift.id, gift.clone()));
}

#[ic_cdk::update]
fn update_gift(id: u64, payload: GiftPayload) -> Result<Gift, Error> {

       // Perform input validation
    if payload.name.is_empty() || payload.description.is_empty() || payload.status.is_empty() {
        return Err(Error::InvalidInput {
            msg: "Invalid payload data Fill all Fields.".to_string(),
        });
    }
    match GIFT_STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut gift) => {
            gift.name = payload.name;
            gift.description = payload.description;
            gift.recipient_id = payload.recipient_id;
            gift.status = payload.status;
            gift.updated_at = Some(time());
            do_insert_gift(&gift);
            Ok(gift)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't update a Gift  with id={}. Gift  not found",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn delete_gift(id: u64) -> Result<Gift, Error> {
    match GIFT_STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(gift) => Ok(gift),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete a Gift with id={}. Gift not found.",
                id
            ),
        }),
    }
}


//Recipient CRUD


// get all Recipients
#[ic_cdk::query]
fn get_all_recipients() -> Result<Vec<Recipient>, Error>{
    let recipients_map: Vec<(u64,Recipient)> = RECIPIENT_STORAGE.with(|service| service.borrow().iter().collect());
    let recipients: Vec<Recipient> = recipients_map.into_iter().map(|(_, recipient)| recipient).collect();

    if !recipients.is_empty(){
        Ok(recipients)
    }else {
        Err(Error::NotFound{
            msg: "No Recipient found.".to_string(),
        })
    }
}


// get the Recipient by Id 
#[ic_cdk::query]
fn get_recipient(id: u64) -> Result<Recipient, Error> {
    match _get_recipient(&id) {
        Some(recipient) => Ok(recipient),
        None => Err(Error::NotFound {
            msg: format!(" Recipient with id={} not found", id),
        }),
    }
}
// a helper method to get a recipient by id.
fn _get_recipient(id: &u64) -> Option<Recipient> {
    RECIPIENT_STORAGE.with(|service| service.borrow().get(id))
}

//Get the most recently created Recipients
#[ic_cdk::query]
fn get_recent_recipients() -> Result<Vec<Recipient>, Error> {
    let recipients_map: Vec<(u64,Recipient)> = RECIPIENT_STORAGE.with(|service| service.borrow().iter().collect());

    // We are Sorting  the Recipients by created_at timestamp in reverse order to get the most recent ones first
    let mut sorted_recipients: Vec<Recipient> = recipients_map
        .into_iter()
        .map(|(_, recipient)| recipient)
        .collect();
    sorted_recipients.sort_by_key(|recipient| Reverse(recipient.created_at));

    // Show the first 5 Recipients recent
    let recent_recipients: Vec<Recipient> = sorted_recipients.into_iter().take(5).collect();

    if !recent_recipients.is_empty() {
        Ok(recent_recipients)
    } else {
        Err(Error::NotFound {
            msg: "No recent Recipients  found.".to_string(),
        })
    }
}


// Create a New Recipient 
#[ic_cdk::update]

fn add_recipient(payload: RecipientPayload) -> Option<Recipient> {

     // Perform input validation
     if payload.name.is_empty() || payload.relationship.is_empty() {
        return None;
    }
    let id = RECIPIENT_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");
    let new_recipient = Recipient { 
        id,
        name: payload.name,
        relationship: payload.relationship,
        created_at: time(), 
        updated_at: None 
    };
    do_insert_recipient(&new_recipient);
    Some(new_recipient)
}

fn do_insert_recipient(recipient: &Recipient) {
    RECIPIENT_STORAGE.with(|service| service.borrow_mut().insert(recipient.id, recipient.clone()));
}

#[ic_cdk::update]
fn update_recipient(id: u64, payload: RecipientPayload) -> Result<Recipient, Error> {
       // Perform input validation
    if payload.name.is_empty() || payload.relationship.is_empty(){
        return Err(Error::InvalidInput {
            msg: "Invalid payload data Fill in the spaces".to_string(),
        });
    }
    match RECIPIENT_STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut recipient) => {
            recipient.name = payload.name;
            recipient.relationship = payload.relationship;
            recipient.updated_at = Some(time());
            do_insert_recipient(&recipient);
            Ok(recipient)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't update a Recipient with id={}. Recipient not found",
                id
            ),
        }),
    }
}


#[ic_cdk::update]
fn delete_recipient(id: u64) -> Result<Recipient, Error> {
    match RECIPIENT_STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(recipient) => Ok(recipient),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete a Recipient with id={}. Recipient not found.",
                id
            ),
        }),
    }
}

// search  Gift by the name or description 
#[ic_cdk::query]
fn search_gift(search_input: String) -> Result<Vec<Gift>, Error> {
    let gifts_map: Vec<(u64,Gift)> = GIFT_STORAGE.with(|service| service.borrow().iter().collect());

      
      // Filter Gifts  by the provided search Input (case insensitive) Implemented
      let searched_gifts: Vec<Gift> = gifts_map
          .into_iter()
          .map(|(_, gift)| gift)
          .filter(|gift| gift.name.to_lowercase() == search_input.to_lowercase() || gift.description.to_lowercase() == search_input.to_lowercase())
          .collect();
  
      if !searched_gifts.is_empty() {
          Ok(searched_gifts)
      } else {
          Err(Error::NotFound {
              msg: format!("No Gifts  found related with the Search Input : {}", search_input),
          })
      }
  }


// Get the last most recent gifts created



#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    InvalidInput { msg: String },
}



// need this to generate candid
ic_cdk::export_candid!();
