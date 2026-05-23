#[cfg(feature = "llvm")]
use azurite_test::run;

#[test]
#[cfg(feature = "llvm")]
fn test_simple_print() {
    run("func main() { print(42) }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_push_pop_len() {
    run("func main() {
        let arr = [1, 2, 3]
        print(arr.len())
        arr.push(42)
        print(arr.len())
        let x = arr.pop()
        print(x)
        print(arr.len())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_pop_empty() {
    run("func main() {
        let arr = [1]
        print(arr.pop())
        print(arr.len())
        arr.pop()
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_is_empty() {
    run("func main() {
        let arr = [1, 2]
        print(arr.is_empty())
        arr.pop()
        arr.pop()
        print(arr.is_empty())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_clear() {
    run("func main() {
        let arr = [1, 2, 3]
        arr.clear()
        print(arr.len())
        print(arr.is_empty())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_contains() {
    run("func main() {
        let arr = [10, 20, 30, 40]
        print(arr.contains(20))
        print(arr.contains(99))
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_reverse() {
    run("func main() {
        let arr = [1, 2, 3, 4, 5]
        arr.reverse()
        print(arr.pop())
        print(arr.pop())
        print(arr.pop())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_sort() {
    run("func main() {
        let arr = [3, 1, 4, 1, 5]
        arr.sort()
        print(arr.pop())
        print(arr.pop())
        print(arr.pop())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_insert() {
    run("func main() {
        let arr = [10, 30]
        arr.insert(1, 20)
        print(arr[0])
        print(arr[1])
        print(arr[2])
        print(arr.len())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_remove() {
    run("func main() {
        let arr = [10, 99, 20]
        arr.remove(1)
        print(arr[0])
        print(arr[1])
        print(arr.len())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_float() {
    run("func main() {
        let arr = [3.14, 2.71, 1.41]
        print(arr[0])
        print(arr[1])
        print(arr[2])
        print(arr.len())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_string() {
    run("func main() {
        let arr = [\"hello\", \"world\"]
        print(arr[0])
        print(arr.len())
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_map() {
    run("func double(x: int) -> int { return x * 2 }
    func main() {
        let arr = [1, 2, 3]
        let m = arr.map(double)
        print(m[0])
        print(m[1])
        print(m[2])
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_filter() {
    run("func is_pos(x: int) -> bool { return x > 0 }
    func main() {
        let arr = [-2, -1, 0, 1, 2]
        let f = arr.filter(is_pos)
        print(f[0])
        print(f[1])
    }").unwrap();
}

#[test]
#[cfg(feature = "llvm")]
fn test_arr_reduce() {
    run("func add(a: int, b: int) -> int { return a + b }
    func main() {
        let arr = [1, 2, 3, 4, 5]
        let s = arr.reduce(0, add)
        print(s)
    }").unwrap();
}
