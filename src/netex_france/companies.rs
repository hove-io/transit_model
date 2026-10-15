// Copyright (C) 2017 Hove and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

use crate::xml_builder::{Element, Node};
use crate::{
    netex_france::exporter::{Exporter, ObjectType},
    objects::{Company, CompanyRole},
    Model,
};

pub struct CompanyExporter<'a> {
    model: &'a Model,
}

// Publicly exposed methods
impl<'a> CompanyExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        CompanyExporter { model }
    }
    pub fn export(&self) -> Vec<Element> {
        self.model
            .companies
            .values()
            .map(|company| self.export_company(company))
            .collect()
    }
}

// Internal methods
impl<'a> CompanyExporter<'a> {
    fn export_company(&self, company: &'a Company) -> Element {
        let element_builder = Element::builder(ObjectType::Operator.to_string())
            .attr(
                "id",
                Exporter::generate_id(&company.id, ObjectType::Operator),
            )
            .attr("version", "any");
        let element_builder =
            element_builder.append_all(Exporter::generate_key_list(&company.codes)); // must be first child (XSD order)
        let element_builder = element_builder.append(self.generate_name(company));
        let element_builder = element_builder.append_all(self.generate_contact_details(company));
        let element_builder = element_builder.append(self.generate_organization_type(company));
        element_builder.build()
    }

    fn generate_name(&self, company: &'a Company) -> Element {
        Element::builder("Name")
            .append(Node::Text(company.name.to_owned()))
            .build()
    }

    fn generate_contact_details(&self, company: &'a Company) -> Option<Element> {
        if company.mail.is_none() && company.phone.is_none() && company.url.is_none() {
            return None;
        }
        let element_builder = Element::builder("ContactDetails");
        let element_builder = if let Some(email_element) = self.generate_email(company) {
            element_builder.append(email_element)
        } else {
            element_builder
        };
        let element_builder = if let Some(phone_element) = self.generate_phone(company) {
            element_builder.append(phone_element)
        } else {
            element_builder
        };
        let element_builder = if let Some(url_element) = self.generate_url(company) {
            element_builder.append(url_element)
        } else {
            element_builder
        };
        Some(element_builder.build())
    }

    fn generate_email(&self, company: &'a Company) -> Option<Element> {
        company.mail.as_ref().map(|email| {
            Element::builder("Email")
                .append(Node::Text(email.to_owned()))
                .build()
        })
    }

    fn generate_phone(&self, company: &'a Company) -> Option<Element> {
        company.phone.as_ref().map(|phone| {
            Element::builder("Phone")
                .append(Node::Text(phone.to_owned()))
                .build()
        })
    }

    fn generate_url(&self, company: &'a Company) -> Option<Element> {
        company.url.as_ref().map(|url| {
            Element::builder("Url")
                .append(Node::Text(url.to_owned()))
                .build()
        })
    }

    fn generate_organization_type(&self, company: &'a Company) -> Element {
        let organisation_type = match company.role {
            CompanyRole::Authority => "authority",
            CompanyRole::Operator => "operator",
        };
        Element::builder("OrganisationType")
            .append(Node::Text(organisation_type.to_owned()))
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelBuilder;

    mod generate_contact_details {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn absent_when_no_email_phone_or_url() {
            let model = ModelBuilder::default()
                .company("C1", |_| {})
                .vj("vj1", |vj| {
                    vj.company("C1").st("A", "10:00:00").st("B", "10:10:00");
                })
                .build();
            let company_exporter = CompanyExporter::new(&model);
            let company = model.companies.get("C1").unwrap();
            assert!(company_exporter.generate_contact_details(company).is_none());
        }

        #[test]
        fn present_with_only_the_fields_that_are_set() {
            let model = ModelBuilder::default()
                .company("C1", |c| {
                    c.mail = Some("contact@example.com".to_string());
                })
                .vj("vj1", |vj| {
                    vj.company("C1").st("A", "10:00:00").st("B", "10:10:00");
                })
                .build();
            let company_exporter = CompanyExporter::new(&model);
            let company = model.companies.get("C1").unwrap();
            let contact_details = company_exporter.generate_contact_details(company).unwrap();
            assert_eq!(contact_details.name(), "ContactDetails");
            let children: Vec<&str> = contact_details
                .nodes()
                .filter_map(|node| node.as_element())
                .map(|element| element.name())
                .collect();
            assert_eq!(children, vec!["Email"]);
        }
    }
}
