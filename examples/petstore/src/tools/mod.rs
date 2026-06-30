use mcp_core::ToolHandle;

pub mod find_pets_by_status;
pub mod get_inventory;
pub mod get_pet_by_id;

pub fn all() -> Vec<ToolHandle> {
    vec![
        get_pet_by_id::get_pet_by_id_tool(),
        find_pets_by_status::find_pets_by_status_tool(),
        get_inventory::get_inventory_tool(),
    ]
}
