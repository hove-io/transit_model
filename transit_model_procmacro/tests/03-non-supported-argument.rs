use transit_model::objects::*;
use transit_model_collection::*;
use transit_model_procmacro::*;
use transit_model_relations::*;

#[derive(GetCorresponding)]
pub struct Model {
    #[get_corresponding(nonsupportedargument)]
    lines_to_routes: OneToMany<Line, Route>,
}

fn main() {}
