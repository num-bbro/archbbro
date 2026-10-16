//! A small example model, used by the CLI, the desktop app's "New example" and tests.

use crate::metamodel::{ElementType::*, RelationshipType::*};
use crate::model::Model;
use std::collections::HashMap;

/// An online shop across the business, application and technology layers, with one view.
pub fn online_shop() -> Model {
    let mut m = Model::new("Online Shop");
    m.purpose = "Example model created by archimate-rs".into();
    let customer = m.add_element(BusinessActor, "Customer");
    let order = m.add_element(BusinessProcess, "Place Order");
    let ordering = m.add_element(BusinessService, "Ordering");
    let webshop = m.add_element(ApplicationComponent, "Web Shop");
    let checkout = m.add_element(ApplicationService, "Checkout");
    let db = m.add_element(SystemSoftware, "PostgreSQL");
    let server = m.add_element(Node, "App Server");

    let rels = [
        (Serving, &ordering, &customer),
        (Realization, &order, &ordering),
        (Serving, &checkout, &order),
        (Realization, &webshop, &checkout),
        (Serving, &db, &webshop),
        (Composition, &server, &db),
    ];
    let rel_ids: Vec<_> = rels
        .iter()
        .map(|(k, s, t)| m.add_relationship(*k, s, t))
        .collect();

    let view = m.add_diagram("Layered Overview");
    let place = [
        (&customer, 40, 30),
        (&ordering, 40, 140),
        (&order, 260, 140),
        (&checkout, 260, 250),
        (&webshop, 480, 250),
        (&db, 480, 360),
        (&server, 700, 360),
    ];
    let mut node_of = HashMap::new();
    for (e, x, y) in place {
        let n = m.add_to_diagram(&view, e, x, y).expect("view exists");
        node_of.insert(e.clone(), n);
    }
    for (rid, (_, s, t)) in rel_ids.iter().zip(rels.iter()) {
        let (sn, tn) = (node_of[*s].clone(), node_of[*t].clone());
        m.connect_in_diagram(&view, rid, &sn, &tn);
    }
    m
}

#[cfg(test)]
mod tests {
    #[test]
    fn example_is_valid() {
        let m = super::online_shop();
        let issues = crate::validate::validate(&m);
        assert!(issues.is_empty(), "{issues:?}");
    }
}
