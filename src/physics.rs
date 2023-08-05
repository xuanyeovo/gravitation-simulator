use std::ops::*;
use num_bigfloat::{ BigFloat, ZERO, ONE, TWO };
use uuid::Uuid;
use std::time::Duration;
use std::fmt::{ Display, Formatter, Result as FmtResult };



/// 表示三维空间中的一个点的坐标
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Point {
    pub x: BigFloat,
    pub y: BigFloat,
    pub z: BigFloat,
}

/// 一个三维向量
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Vector {
    pub x: BigFloat,
    pub y: BigFloat,
    pub z: BigFloat,
}

#[derive(Clone, Debug)]
pub struct PhysicalAttributes {
    /// 物体的重心
    pub center: Point,

    /// 物体的速度，以m/s(米每秒)为单位
    pub velocity: Vector,

    /// 物体受到的力，以N(牛顿)为单位
    ///
    /// 在计算中会和`mass`参与计算加速度
    pub force: Vector,

    /// 物体的质量，以Kg为单位
    ///
    /// 在计算中会和`force`参与计算加速度
    pub mass: BigFloat,
}

pub struct Objects<'a: 'this, 'this> {
    inner: Vec<&'a mut dyn PhysicalObject>,
    _marker: std::marker::PhantomData<&'this Self>,
}

#[derive(Debug, Default)]
pub struct SpaceExecutor {}



impl Display for PhysicalAttributes {
    fn fmt(&self, formatter: &mut Formatter<'_>)-> FmtResult {
        write!(formatter, r#"
  Center:
{}
  Velocity:
{}
    {}m/s
  Force:
{}
    {}N
  Mass: {}
"#, self.center, self.velocity, self.velocity.model(), self.force, self.force.model(), self.mass)
    }
}

impl Add<Vector> for Vector {
    type Output = Self;

    fn add(self, other: Vector)-> Self::Output {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl AddAssign<Vector> for Vector {
    fn add_assign(&mut self, other: Vector) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }
}

impl Mul<BigFloat> for Vector {
    type Output = Self;

    fn mul(self, other: BigFloat)-> Self::Output {
        Self {
            x: self.x * other,
            y: self.y * other,
            z: self.z * other,
        }
    }
}

impl Display for Vector {
    fn fmt(&self, formatter: &mut Formatter<'_>)-> FmtResult {
        write!(formatter, r#"
   ({},
    {},
    {})"#, self.x, self.y, self.z)
    }
}

impl Display for Point {
    fn fmt(&self, formatter: &mut Formatter<'_>)-> FmtResult {
        write!(formatter, r#"
   ({},
    {},
    {})"#, self.x, self.y, self.z)
    }
}

impl<'a, 'it> Objects<'a, 'it> {
    pub fn new(objects: Vec<&'a mut dyn PhysicalObject>)-> Objects<'a, 'it> {
        Self {
            inner: objects,
            _marker: std::marker::PhantomData::<&'it Self>::default(),
        }
    }
}

impl Executor for SpaceExecutor {
    fn execute_force(&mut self, objects: &mut Objects, _time: Duration) {
        // 计算每个物体所受引力情况
        // 根据万有引力公式进行计算
        // F = (G * m1 * m2) / (r^2)
        // 引力常数G取6.67259 x 10^-11 (m^3 / (kg * s^2))
        #[allow(non_snake_case)]
        let G = "6.67259e-11".parse::<BigFloat>().unwrap();
        let mut forces = Vec::new();

        for object1 in objects.iter() {

            let attr1 = (*object1).get_physical_attributes();

            let mut final_force = Vector { x:ZERO, y:ZERO, z:ZERO };

            for object2 in objects.iter().filter(|i| (**i).get_uid() != (*object1).get_uid()) {

                let attr2 = (*object2).get_physical_attributes();

                let r = attr1.center.distance(&attr2.center);
                if r == ZERO {
                    continue;
                }

                let force_size = (G * attr1.mass * attr2.mass) / (r.pow(&TWO));
                let f = attr1.center.unit_vector_to(&attr2.center) * force_size;
                final_force += f;

            }

            forces.push(final_force);
        }

        objects
            .iter_mut()
            .zip(forces.iter())
            .for_each(|(obj, force)| (**obj).get_physical_attributes_mut().force = *force);
    }

    fn execute_displacement(&mut self, objects: &mut Objects, time: Duration) {
        for current_object in objects.iter_mut() {
            let attr = (*current_object).get_physical_attributes_mut();
            let t = BigFloat::from(time.as_micros()) / BigFloat::from(1e6);
            let acceleration = attr.force * (ONE / attr.mass);
            let displacement = attr.velocity * t + acceleration * t.pow(&TWO) * BigFloat::from(0.5);

            attr.center += displacement;
            attr.velocity += acceleration * t;
        }
    }
}

impl Point {
    /// 计算两点间的距离
    pub fn distance(&self, other: &Point)-> BigFloat {
        let x_sq = (self.x - other.x).pow(&TWO);
        let y_sq = (self.y - other.y).pow(&TWO);
        let z_sq = (self.z - other.z).pow(&TWO);
        (x_sq + y_sq + z_sq).sqrt()
    }

    /// 获取以自身为起点，`other`点为终点的向量
    pub fn vector_to(&self, other: &Point)-> Vector {
        Vector {
            x: other.x - self.x,
            y: other.y - self.y,
            z: other.z - self.z,
        }
    }

    /// 获取到`other`点的方向上的单位向量
    ///
    /// Panics:
    /// 如果该点与原来的点在同一位置上，则会触发panic，因为向量模为0
    pub fn unit_vector_to(&self, other: &Point)-> Vector {
        let v = self.vector_to(other);
        v * (ONE / v.model())
    }
}

impl Add<Vector> for Point {
    type Output = Self;

    fn add(self, v: Vector)-> Self {
        Self {
            x: self.x + v.x,
            y: self.y + v.y,
            z: self.z + v.z,
        }
    }
}

impl AddAssign<Vector> for Point {
    fn add_assign(&mut self, v: Vector) {
        self.x += v.x;
        self.y += v.y;
        self.z += v.z;
    }
}

impl Vector {
    pub fn model(&self)-> BigFloat {
        (self.x.pow(&TWO) + self.y.pow(&TWO) + self.z.pow(&TWO)).sqrt()
    }

    pub const ZERO: Self = Self { x:ZERO, y:ZERO, z:ZERO };
}

impl<'a> Deref for Objects<'a, '_> {
    type Target = Vec<&'a mut dyn PhysicalObject>;

    fn deref(&self)-> &Self::Target {
        &self.inner
    }
}

impl<'a> DerefMut for Objects<'a, '_> {
    fn deref_mut(&mut self)-> &mut Self::Target {
        &mut self.inner
    }
}



/// 实现该trait的结构体可以进行物理运算
///
/// 在把物体添加到`Executor`中进行计算的时候，该物体会被转换为此trait object
pub trait PhysicalObject {
    /// 获得物体的唯一标识符
    ///
    /// 该返回值在整个计算中应为不变,唯一的
    fn get_uid(&self)-> Uuid;

    /// 获得物体的物理属性引用
    fn get_physical_attributes(&self)-> &PhysicalAttributes;

    /// 获得物体的物理属性的可变引用
    fn get_physical_attributes_mut(&mut self)-> &mut PhysicalAttributes;
}

/// 实现该trait可以用于执行物理计算
pub trait Executor {
    /// 计算受力
    fn execute_force(&mut self, objects: &mut Objects, time: Duration);

    /// 计算速度与位移
    fn execute_displacement(&mut self, objects: &mut Objects, time: Duration);
}
