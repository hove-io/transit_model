// Copyright (C) 2017 Kisio Digital and/or its affiliates.
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

use crate::{
    netex_france::{
        exporter::{Exporter, ObjectType},
        NETEX_NS,
    },
    objects::Company,
    Model,
};
use minidom::{Element, Node};

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
        let element_builder = Element::builder(ObjectType::Operator.to_string(), NETEX_NS)
            .attr(
                "id",
                Exporter::generate_id(&company.id, ObjectType::Operator),
            )
            .attr("version", "any");
        let element_builder = element_builder.append(self.generate_name(company));
        let element_builder = element_builder.append(self.generate_contact_details(company));
        let element_builder = element_builder.append(Self::generate_organization_type());
        element_builder.build()
    }

    fn generate_name(&self, company: &'a Company) -> Element {
        Element::builder("Name", NETEX_NS)
            .append(Node::Text(company.name.to_owned()))
            .build()
    }

    fn generate_contact_details(&self, company: &'a Company) -> Element {
        let element_builder = Element::builder("ContactDetails", NETEX_NS);
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
        element_builder.build()
    }

    fn generate_email(&self, company: &'a Company) -> Option<Element> {
        company.mail.as_ref().map(|email| {
            Element::builder("Email", NETEX_NS)
                .append(Node::Text(email.to_owned()))
                .build()
        })
    }

    fn generate_phone(&self, company: &'a Company) -> Option<Element> {
        company.phone.as_ref().map(|phone| {
            Element::builder("Phone", NETEX_NS)
                .append(Node::Text(phone.to_owned()))
                .build()
        })
    }

    fn generate_url(&self, company: &'a Company) -> Option<Element> {
        company.url.as_ref().map(|url| {
            Element::builder("Url", NETEX_NS)
                .append(Node::Text(url.to_owned()))
                .build()
        })
    }

    fn generate_organization_type() -> Element {
        Element::builder("OrganisationType", NETEX_NS)
            .append(Node::Text(String::from("other")))
            .build()
    }
}
