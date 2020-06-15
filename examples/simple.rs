#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Book {
    title: String,
    page_count: u32,
    description: Option<String>,
}

impl house::query::Queryable for Book {
    fn query_terms(&self) -> Vec<house::query::Term> {
        vec![
            house::query::Term { field: "title", value: self.title.as_bytes() },
        ]
    }
}

fn main() {

    let db = sled::Config::new().temporary(true).open().unwrap();

    let tree = db.open_tree(b"books").unwrap();
    let meta = db.open_tree(b"books_meta").unwrap();

    let store: house::Store<Book> = house::Store {
        db: db.clone(), tree, meta, marker: std::marker::PhantomData
    };

    let book = Book {
        title: "The Great Gatsby".into(),
        page_count: 200,
        description: Some("About a man and some other stuff".into()),
    };

    let id = store.create(&book).unwrap();

    println!("{:?}", store.find(id).unwrap());

    let mut book = house::Object { id, inner: book };

    book.title = "The Greatest".into();

    store.update(&book).unwrap();

    let found_books = store
        .filter(house::query::StrEquals("title", "The Greatest"))
        .unwrap()
        .first()
        .unwrap();

    println!("{:?}", &found_books);

}