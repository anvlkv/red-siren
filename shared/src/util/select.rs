#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Select<T> {
    pub options: Vec<T>,
    pub selection: Option<usize>
}

impl<T> From<impl Iterator<Item = T>> for Select<T> {
    fn from(value: impl Iterator<Item = T>) -> Self {
        Self{
          selection: None,
          options: value.collect()
        }
    }
}

impl<T> Select<T> where T: PartialEq {
  pub fn value(&self) -> Option<&T> {
    self.selection.iter().filter_map(|u| self.options.iter().nth(u))
  }
  pub fn select(&mut self, value: &T) {
    self.selection = self.options.iter().position(|v| v == value);
  }
  pub fn with_first_default(self) -> Self {
    Self {
      selection: self.options.first().map(|_| 0),
      options: self.options.clone()
    }
  }
}
