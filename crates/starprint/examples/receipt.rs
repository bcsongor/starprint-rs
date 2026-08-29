//! Prints the Python GUI's till receipt on either printer. The `Magnify`
//! trait papers over the two builders' size APIs and column counts. Two
//! thermal-only styles (`style=a|b`) show off the fonts, inverse printing
//! and CP437 rules.
//!
//! Usage: cargo run --example receipt -- <printer-host-or-ip> thermal|impact [slow] [density=N] [style=a|b]

use starprint::transport::TcpTransport;
use starprint::{
    Alignment, Builder, CodePage, Cut, Document, Impact, PrintSpeed, Protocol, StarLine,
};

const CURRENCY: &str = "£";
const SERVICE_CHARGE_PCT: f64 = 12.5;
const STAMP: &str = "27/08/2026 12:42 PM";

struct Item {
    qty: u32,
    name: &'static str,
    price: f64,
}

const ITEMS: &[Item] = &[
    Item {
        qty: 2,
        name: "Bacon Naan Roll",
        price: 8.50,
    },
    Item {
        qty: 1,
        name: "House Black Daal",
        price: 9.90,
    },
    Item {
        qty: 1,
        name: "Chicken Ruby",
        price: 14.90,
    },
    Item {
        qty: 2,
        name: "Garlic Naan",
        price: 3.70,
    },
    Item {
        qty: 1,
        name: "Okra Fries",
        price: 5.90,
    },
    Item {
        qty: 2,
        name: "Chai",
        price: 3.20,
    },
];

trait Magnify: Sized {
    const COLUMNS: usize;

    fn set_wide(self, on: bool) -> Self;
    fn set_tall(self, on: bool) -> Self;
}

/// TSP800II with the print width set to 80 mm: 576 dots / 12-dot font A.
impl Magnify for Builder<StarLine> {
    const COLUMNS: usize = 48;

    fn set_wide(self, on: bool) -> Self {
        self.wide(if on { 2 } else { 1 })
    }

    fn set_tall(self, on: bool) -> Self {
        self.tall(if on { 2 } else { 1 })
    }
}

/// SP700 in its default 7×9 font.
impl Magnify for Builder<Impact> {
    const COLUMNS: usize = 42;

    fn set_wide(self, on: bool) -> Self {
        self.double_wide(on)
    }

    fn set_tall(self, on: bool) -> Self {
        self.double_tall(on)
    }
}

/// Left text on the left, right text flush to column `width`, the gap
/// filled with `fill`.
fn two_columns_with(left: &str, right: &str, width: usize, fill: char) -> String {
    let gap = width.saturating_sub(left.chars().count() + right.chars().count());
    format!("{left}{}{right}", fill.to_string().repeat(gap))
}

fn two_columns(left: &str, right: &str, width: usize) -> String {
    two_columns_with(left, right, width, ' ')
}

fn money(amount: f64) -> String {
    format!("{CURRENCY}{amount:.2}")
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn rule(c: char, width: usize) -> String {
    c.to_string().repeat(width)
}

struct Totals {
    subtotal: f64,
    service_charge: f64,
    total: f64,
}

fn totals() -> Totals {
    let subtotal: f64 = ITEMS.iter().map(|i| f64::from(i.qty) * i.price).sum();
    let service_charge = subtotal * SERVICE_CHARGE_PCT / 100.0;
    Totals {
        subtotal,
        service_charge,
        total: subtotal + service_charge,
    }
}

fn item_line(item: &Item, name: &str, width: usize, fill: char) -> String {
    let right = money(f64::from(item.qty) * item.price);
    let max_left = width.saturating_sub(right.chars().count() + 1).max(1);
    let left = truncate(&format!("{:>2} {name}", item.qty), max_left);
    if fill == ' ' {
        two_columns(&left, &right, width)
    } else {
        two_columns_with(&format!("{left} "), &format!(" {right}"), width, fill)
    }
}

/// The GUI layout, for either printer.
fn receipt<P: Protocol>(builder: Builder<P>) -> Document
where
    Builder<P>: Magnify,
{
    let width = <Builder<P> as Magnify>::COLUMNS;
    let wide_width = width / 2;

    let mut receipt = builder
        .code_page(CodePage::CP437)
        .align(Alignment::Center)
        // First header line is emphasised and quad size.
        .bold(true)
        .set_wide(true)
        .set_tall(true)
        .line(&truncate("DISHOOM", wide_width))
        .set_tall(false)
        .set_wide(false)
        .bold(false)
        .line("13 Water Street, Canary Wharf")
        .line("London E14 5GX")
        .line("020 7420 9326")
        .feed(1)
        .align(Alignment::Right)
        .line(STAMP)
        .align(Alignment::Left)
        .feed(1);

    for item in ITEMS {
        receipt = receipt.line(&item_line(item, &item.name.to_uppercase(), width, ' '));
        if item.qty > 1 {
            receipt = receipt.line(&format!("   @ {:.2} each", item.price));
        }
    }

    let t = totals();
    receipt
        .feed(1)
        .line(&two_columns("Subtotal", &money(t.subtotal), width))
        .line(&two_columns(
            &format!("Service charge ({SERVICE_CHARGE_PCT}%)"),
            &money(t.service_charge),
            width,
        ))
        .feed(1)
        .bold(true)
        .set_wide(true)
        .line(&two_columns("TOTAL", &money(t.total), wide_width))
        .set_wide(false)
        .bold(false)
        .feed(1)
        .align(Alignment::Center)
        .line("Table 12 - 2 covers")
        .line("Service charge goes to the team")
        .line("Thank you, come again")
        .align(Alignment::Left)
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build()
}

#[derive(Clone, Copy)]
enum Style {
    /// Letter-spaced wordmark and ruled sections.
    Classic,
    /// Black bands with knocked-out text, dotted price leaders.
    Band,
}

/// Option A: letter-spaced wordmark, double and single rules.
fn classic(b: Builder<StarLine>) -> Builder<StarLine> {
    const W: usize = <Builder<StarLine> as Magnify>::COLUMNS;
    let t = totals();
    let mut b = b
        .align(Alignment::Center)
        .feed(1)
        .bold(true)
        .wide(2)
        .tall(2)
        .line("D I S H O O M")
        .wide(1)
        .tall(1)
        .bold(false)
        .feed(1)
        .line("13 Water Street, Canary Wharf")
        .line("London E14 5GX")
        .feed(1)
        .align(Alignment::Left)
        .line(&rule('═', W))
        .line(&two_columns("Table 12  ·  2 covers", STAMP, W))
        .line(&rule('─', W));

    for item in ITEMS {
        b = b.line(&item_line(item, &item.name.to_uppercase(), W, ' '));
        if item.qty > 1 {
            b = b.line(&format!("   @ {:.2} each", item.price));
        }
    }

    b.line(&rule('─', W))
        .line(&two_columns("Subtotal", &money(t.subtotal), W))
        .line(&two_columns(
            &format!("Service charge ({SERVICE_CHARGE_PCT}%)"),
            &money(t.service_charge),
            W,
        ))
        .line(&rule('═', W))
        .bold(true)
        .wide(2)
        .line(&two_columns("TOTAL", &money(t.total), W / 2))
        .wide(1)
        .bold(false)
        .line(&rule('═', W))
        .feed(1)
        .align(Alignment::Center)
        .line("Thank you, come again")
        .line("dishoom.com  ·  020 7420 9326")
        .align(Alignment::Left)
}

/// Option B: a wide wordmark, dotted leaders and an inverse total.
fn band(b: Builder<StarLine>) -> Builder<StarLine> {
    const W: usize = <Builder<StarLine> as Magnify>::COLUMNS;
    let t = totals();
    let mut b = b
        .align(Alignment::Center)
        .feed(1)
        .bold(true)
        .wide(2)
        .tall(2)
        .line("DISHOOM")
        .tall(1)
        .wide(1)
        .bold(false)
        .feed(1)
        .line("Canary Wharf")
        .line("13 Water Street  ·  E14 5GX")
        .feed(1)
        .align(Alignment::Left)
        .line(&two_columns("Table 12", STAMP, W))
        .feed(1);

    for item in ITEMS {
        b = b.line(&item_line(item, &item.name.to_uppercase(), W, '.'));
        if item.qty > 1 {
            b = b.line(&format!("   @ {:.2} each", item.price));
        }
    }

    b.feed(1)
        .line(&two_columns("Subtotal", &money(t.subtotal), W))
        .line(&two_columns(
            &format!("Service charge ({SERVICE_CHARGE_PCT}%)"),
            &money(t.service_charge),
            W,
        ))
        .feed(1)
        .invert(true)
        .bold(true)
        .wide(2)
        .line(&two_columns(
            " TOTAL",
            &format!("{} ", money(t.total)),
            W / 2,
        ))
        .wide(1)
        .bold(false)
        .invert(false)
        .feed(1)
        .align(Alignment::Center)
        .line("·  Est. 2010  ·")
        .line("Thank you, come again")
        .align(Alignment::Left)
}

fn styled(builder: Builder<StarLine>, style: Style) -> Document {
    let b = builder.code_page(CodePage::CP437);
    let b = match style {
        Style::Classic => classic(b),
        Style::Band => band(b),
    };
    b.feed(2).cut(Cut::FeedThenPartial).build()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const USAGE: &str =
        "usage: receipt <printer-host-or-ip> thermal|impact [slow] [density=N] [style=a|b]";
    let mut args = std::env::args().skip(1);
    let host = args.next().ok_or(USAGE)?;
    let kind = args.next().ok_or(USAGE)?;
    let flags: Vec<String> = args.collect();
    let slow = flags.iter().any(|a| a == "slow");
    let density = flags
        .iter()
        .find_map(|a| a.strip_prefix("density=").map(str::parse::<i8>))
        .transpose()?;
    let style = match flags
        .iter()
        .find_map(|a| a.strip_prefix("style="))
        .unwrap_or("")
    {
        "" => None,
        "a" => Some(Style::Classic),
        "b" => Some(Style::Band),
        _ => return Err(USAGE.into()),
    };
    let doc = match kind.as_str() {
        "thermal" => {
            let mut builder = starprint::starline();
            if slow {
                builder = builder.print_speed(PrintSpeed::Slow);
            }
            if let Some(level) = density {
                builder = builder.print_density(level);
            }
            match style {
                Some(style) => styled(builder, style),
                None => receipt(builder),
            }
        }
        "impact" if style.is_none() => receipt(starprint::impact()),
        "impact" => return Err("style= is only available for thermal printers".into()),
        _ => return Err(USAGE.into()),
    };

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!("sent {} bytes to {host}", doc.as_bytes().len());
    Ok(())
}
